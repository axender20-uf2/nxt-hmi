"use client";

import { useState, useEffect } from "react";
import {
  WifiOff,
  TrendingUp,
  TrendingDown,
  Wifi,
  Server,
  Volume2,
  VolumeX,
  Moon,
  Sun,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

interface Alert {
  id: string;
  dateTime: string;
  type: "disconnect" | "tempUp" | "tempDown";
  device: string;
  description: string;
}

interface AlertRemovalEvent {
  id: string;
}

interface MuteStatePayload {
  muted: boolean;
  expiresAt?: string | null;
}

const getAlertTypeInfo = (type: Alert["type"]) => {
  switch (type) {
    case "disconnect":
      return {
        icon: WifiOff,
        label: "Desconexión",
        color: "text-[#EF4444]",
      };
    case "tempUp":
      return {
        icon: TrendingUp,
        label: "Aumento temp.",
        color: "text-[#F97316]",
      };
    case "tempDown":
      return {
        icon: TrendingDown,
        label: "Disminución temp.",
        color: "text-[#3B82F6]",
      };
  }
};

export function TemperatureDashboard() {
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [currentTime, setCurrentTime] = useState(new Date());
  const [isInternetConnected, setIsInternetConnected] = useState(true);
  const [isServerConnected, setIsServerConnected] = useState(false);
  const [isMuted, setIsMuted] = useState(false);
  const [muteExpiresAt, setMuteExpiresAt] = useState<string | null>(null);
  const [isDarkMode, setIsDarkMode] = useState(false);
  const [showAbout, setShowAbout] = useState(false);

  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(new Date());
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const loadAlertsFromRust = async () => {
      try {
        const result = await invoke<Alert[]>("get_active_alerts");
        setAlerts(result);
      } catch (error) {
        console.error("Error al cargar alertas desde Rust:", error);
      }
    };

    loadAlertsFromRust();
  }, []);

  useEffect(() => {
    const loadMuteState = async () => {
      try {
        const result = await invoke<MuteStatePayload>("get_mute_status");
        setIsMuted(result.muted);
        setMuteExpiresAt(result.expiresAt ?? null);
      } catch (error) {
        console.error("Error al cargar estado de mute:", error);
      }
    };

    loadMuteState();
  }, []);

  useEffect(() => {
    let unlistenAdded: UnlistenFn | null = null;
    let unlistenRemoved: UnlistenFn | null = null;
    let cancelled = false;

    const registerListeners = async () => {
      try {
        unlistenAdded = await listen<Alert>("alerts://added", (event) => {
          setAlerts((prev) => {
            const filtered = prev.filter(
              (alert) => alert.id !== event.payload.id
            );
            return [event.payload, ...filtered];
          });
        });

        unlistenRemoved = await listen<AlertRemovalEvent>(
          "alerts://removed",
          (event) => {
            setAlerts((prev) =>
              prev.filter((alert) => alert.id !== event.payload.id)
            );
          }
        );
      } catch (error) {
        if (!cancelled) {
          console.error("Error al registrar listeners de alertas:", error);
        }
      }
    };

    registerListeners();

    return () => {
      cancelled = true;
      unlistenAdded?.();
      unlistenRemoved?.();
    };
  }, []);

  useEffect(() => {
    let unlistenMute: UnlistenFn | null = null;
    let cancelled = false;

    const registerMuteListener = async () => {
      try {
        unlistenMute = await listen<MuteStatePayload>(
          "alerts://mute_changed",
          (event) => {
            if (cancelled) return;
            setIsMuted(event.payload.muted);
            setMuteExpiresAt(event.payload.expiresAt ?? null);
          }
        );
      } catch (error) {
        if (!cancelled) {
          console.error("Error al registrar listener de mute:", error);
        }
      }
    };

    registerMuteListener();

    return () => {
      cancelled = true;
      unlistenMute?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const pollConnection = async () => {
      try {
        const result = await invoke<boolean>("check_internet_connection");
        if (!cancelled) {
          setIsInternetConnected(result);
        }
      } catch (error) {
        if (!cancelled) {
          console.error("Error al comprobar conexión a Internet:", error);
          setIsInternetConnected(false);
        }
      }
    };

    // Primera comprobación inmediata
    pollConnection();
    const interval = setInterval(pollConnection, 1000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const pollServerConnection = async () => {
      try {
        const result = await invoke<boolean>("is_mqtt_connected");
        if (!cancelled) {
          setIsServerConnected(result);
        }
      } catch (error) {
        if (!cancelled) {
          console.error("Error al comprobar conexión del servidor:", error);
          setIsServerConnected(false);
        }
      }
    };

    pollServerConnection();
    const interval = setInterval(pollServerConnection, 1000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const handleDeleteAlert = async (id: string) => {
    try {
      const removed = await invoke<boolean>("remove_alert", { id });
      if (removed) {
        setAlerts((prev) => prev.filter((alert) => alert.id !== id));
      }
    } catch (error) {
      console.error("Error al eliminar alerta:", error);
    }
  };

  const handleToggleMute = async () => {
    try {
      const result = await invoke<MuteStatePayload>("toggle_alerts_mute");
      setIsMuted(result.muted);
      setMuteExpiresAt(result.expiresAt ?? null);
    } catch (error) {
      console.error("Error al alternar mute:", error);
    }
  };

  const formatDateTime = (date: Date) => {
    const daysShort = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
    const monthsShort = [
      "ene.",
      "feb.",
      "mar.",
      "abr.",
      "may.",
      "jun.",
      "jul.",
      "ago.",
      "sep.",
      "oct.",
      "nov.",
      "dic.",
    ];

    const dayName = daysShort[date.getDay()];
    const dayNum = date.getDate();
    const monthName = monthsShort[date.getMonth()];
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");

    return `${dayName} ${dayNum} ${monthName} ${hours}:${minutes}`;
  };

  const muteButtonDisabled = !isMuted && alerts.length === 0;
  const muteTooltip = isMuted
    ? muteExpiresAt
      ? `Silenciado hasta ${new Date(muteExpiresAt).toLocaleTimeString()}`
      : "Silencio temporal activo"
    : muteButtonDisabled
    ? "Sin alertas activas"
    : "Silenciar";

  return (
    <div
      className="flex h-screen flex-col px-8 py-6"
      style={{
        backgroundColor: isDarkMode ? "#020617" : "#0b0fbe5e",
      }}
    >
      <div className="mb-4 grid w-full grid-cols-3 items-center px-4 py-3">
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            onClick={() => setShowAbout(true)}
            className="flex items-center gap-2 text-white/90 hover:bg-white/10 transition-colors px-2"
          >
            <span className="text-base">Acerca de</span>
          </Button>

          <span className="text-base font-semibold text-white">
            Panel HMI
          </span>
        </div>

        <div className="flex justify-center">
          <span className="text-base font-medium text-white/90">
            {formatDateTime(currentTime)}
          </span>
        </div>

        <div className="flex items-center justify-end gap-6">
          <div className="flex items-center gap-5">
            <button
              className="group transition-all"
              title={
                isInternetConnected
                  ? "Internet conectado"
                  : "Internet desconectado"
              }
            >
              {isInternetConnected ? (
                <Wifi className="h-9 w-9 text-white transition-transform duration-300 group-hover:scale-110" />
              ) : (
                <WifiOff className="h-9 w-9 text-red-500 animate-pulse" />
              )}
            </button>

            <button
              className="group transition-all"
              title={
                isServerConnected
                  ? "Servidor conectado"
                  : "Servidor desconectado"
              }
            >
              <Server
                className={
                  isServerConnected
                    ? "h-9 w-9 text-white transition-transform duration-300 group-hover:scale-110"
                    : "h-9 w-9 text-red-500 animate-pulse"
                }
              />
            </button>

            <button
              onClick={() => setIsDarkMode(!isDarkMode)}
              className="group transition-all"
            >
              {isDarkMode ? (
                <Sun className="h-9 w-9 text-white" />
              ) : (
                <Moon className="h-9 w-9 text-white" />
              )}
            </button>
          </div>

          <button
            onClick={handleToggleMute}
            className={`text-white/90 transition-all hover:text-white hover:scale-110 active:scale-95 ${
              muteButtonDisabled
                ? "opacity-40 cursor-not-allowed hover:scale-100"
                : ""
            }`}
            title={muteTooltip}
            disabled={muteButtonDisabled}
          >
            {isMuted ? (
              <VolumeX className="h-10 w-10" />
            ) : (
              <Volume2 className="h-10 w-10" />
            )}
          </button>
        </div>
      </div>

      {showAbout && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={() => setShowAbout(false)}
        >
          <div
            className="relative rounded-xl shadow-2xl p-6 max-w-md w-full mx-4"
            style={{
              backgroundColor: isDarkMode ? "#0B1220" : "#ffffff",
              color: isDarkMode ? "#E5E7EB" : "#111827",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={() => setShowAbout(false)}
              className="absolute top-4 right-4 rounded-lg p-1 transition-colors hover:bg-gray-200 dark:hover:bg-gray-700"
              aria-label="Cerrar"
            >
              <X
                className="h-5 w-5"
                style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
              />
            </button>

            <div className="space-y-3 text-sm">
              <h2 className="text-lg font-bold">SEIMEG – HMI Panel</h2>
              <p>
                <span className="font-semibold">Versión:</span> 1.0.1
              </p>
              <p>
                <span className="font-semibold">Desarrollador:</span> axender20
                ame00hdz jorge (Git)
              </p>
              <p>Quetzaltenango, Guatemala – 2025</p>
              <p className="pt-2">
                Sistema de monitoreo y control de temperatura.
              </p>
              <p
                className="pt-2 text-xs"
                style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
              >
                © 2025 SEIMEG. Todos los derechos reservados.
              </p>
            </div>
          </div>
        </div>
      )}

      <div
        className="flex-1 overflow-hidden rounded-2xl shadow-lg mt-4"
        style={{
          backgroundColor: isDarkMode ? "#0B1220" : "#ffffff",
        }}
      >
        {alerts.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center pt-8">
            <img
              src="/images/sistemen-20xdxd.png"
              alt="Sistema OK"
              style={{
                height: "95%",
                width: "auto",
                maxHeight: "100%",
                display: "block",
                marginLeft: "auto",
                marginRight: "auto",
                objectFit: "contain",
              }}
              className="w-auto"
            />
          </div>
        ) : (
          <div className="h-full overflow-auto">
            <table className="w-full">
              <thead className="sticky top-0">
                <tr
                  className="border-b-2"
                  style={{
                    backgroundColor: isDarkMode ? "#0B1220" : "#eeeff8",
                    borderColor: isDarkMode ? "#1F2937" : "#e5e7eb",
                  }}
                >
                  <th className="w-[10%] px-6 py-3"></th>
                  <th
                    className="w-[20%] px-6 py-3 text-left text-xs font-medium uppercase tracking-wide"
                    style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                  >
                    Dispositivo
                  </th>
                  <th
                    className="w-[20%] px-6 py-3 text-left text-xs font-medium uppercase tracking-wide"
                    style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                  >
                    Fecha y Hora
                  </th>
                  <th
                    className="w-[25%] px-6 py-3 text-left text-xs font-medium uppercase tracking-wide"
                    style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                  >
                    Tipo de Alerta
                  </th>
                  <th
                    className="w-[25%] px-6 py-3 text-left text-xs font-medium uppercase tracking-wide"
                    style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                  >
                    Descripción
                  </th>
                </tr>
              </thead>
              <tbody>
                {alerts.map((alert) => {
                  const alertInfo = getAlertTypeInfo(alert.type);
                  const AlertIcon = alertInfo.icon;

                  return (
                    <tr
                      key={alert.id}
                      className="border-b-2 transition-colors"
                      style={{
                        borderColor: isDarkMode ? "#1F2937" : "#e5e7eb",
                        backgroundColor: isDarkMode ? "#0B1220" : "#ffffff",
                      }}
                    >
                      <td className="w-[10%] px-6 py-3">
                        <button
                          onClick={() => handleDeleteAlert(alert.id)}
                          className="rounded-lg px-4 py-2 transition-all hover:opacity-80 hover:shadow-md active:scale-95"
                          aria-label="Eliminar alerta"
                        >
                          <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="18"
                            height="18"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="#EF4444"
                            strokeWidth="2"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          >
                            <path d="M3 6h18" />
                            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                          </svg>
                        </button>
                      </td>
                      <td className="w-[20%] px-6 py-3 text-left">
                        <span
                          className="text-sm font-medium"
                          style={{ color: isDarkMode ? "#E5E7EB" : "#000000" }}
                        >
                          {alert.device}
                        </span>
                      </td>
                      <td className="w-[20%] px-6 py-3 text-left">
                        <span
                          className="text-sm font-medium"
                          style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                        >
                          {alert.dateTime}
                        </span>
                      </td>
                      <td className="w-[25%] px-6 py-3 text-left">
                        <div className="flex items-center gap-2">
                          <AlertIcon className={`h-5 w-5 ${alertInfo.color}`} />
                          <span
                            className={`text-sm font-medium ${alertInfo.color}`}
                          >
                            {alertInfo.label}
                          </span>
                        </div>
                      </td>
                      <td className="w-[25%] px-6 py-3 text-left">
                        <span
                          className="text-sm"
                          style={{ color: isDarkMode ? "#9CA3AF" : "#6B7280" }}
                        >
                          {alert.description}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

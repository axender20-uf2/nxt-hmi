"use client";

import { useState, useEffect } from "react";
import {
  ChevronDown,
  WifiOff,
  TrendingUp,
  TrendingDown,
  Settings,
  Wifi,
  Server,
  Volume2,
  VolumeX,
  Moon,
  Sun,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

// Tipos de datos para sensores
interface Sensor {
  id: string;
  name: string;
  groupType: string;
  activeSensors: number;
  inactiveSensors: number;
  dataPerMonth: number;
  spentPerMonth: string;
  batteryLevel: number;
  lastModified: string;
  isActive: boolean;
  chartData: number[];
}

// Datos de ejemplo (mock data)
const mockSensors: Sensor[] = [
  {
    id: "1",
    name: "Zona A - Almacén",
    groupType: "Refrigeración",
    activeSensors: 0,
    inactiveSensors: 67,
    dataPerMonth: 567,
    spentPerMonth: "$2,746.75",
    batteryLevel: 12,
    lastModified: "03/07/2019",
    isActive: true,
    chartData: [22, 24, 23, 25, 24, 22, 23, 24, 25, 23],
  },
  {
    id: "2",
    name: "Zona B - Producción",
    groupType: "Ambiente",
    activeSensors: 5363,
    inactiveSensors: 67,
    dataPerMonth: 567,
    spentPerMonth: "$2,746.75",
    batteryLevel: 12,
    lastModified: "03/07/2019",
    isActive: true,
    chartData: [18, 19, 21, 20, 19, 18, 20, 21, 19, 20],
  },
  {
    id: "3",
    name: "Zona C - Laboratorio",
    groupType: "Criogénico",
    activeSensors: 5363,
    inactiveSensors: 67,
    dataPerMonth: 567,
    spentPerMonth: "$2,746.75",
    batteryLevel: 12,
    lastModified: "03/07/2019",
    isActive: true,
    chartData: [-80, -78, -82, -79, -80, -81, -79, -80, -78, -82],
  },
  {
    id: "4",
    name: "Zona D - Oficinas",
    groupType: "Ambiente",
    activeSensors: 5363,
    inactiveSensors: 67,
    dataPerMonth: 567,
    spentPerMonth: "$2,746.75",
    batteryLevel: 12,
    lastModified: "03/07/2019",
    isActive: false,
    chartData: [21, 22, 21, 23, 22, 21, 22, 23, 21, 22],
  },
  {
    id: "5",
    name: "Zona E - Exterior",
    groupType: "Ambiente",
    activeSensors: 5363,
    inactiveSensors: 67,
    dataPerMonth: 567,
    spentPerMonth: "$2,746.75",
    batteryLevel: 12,
    lastModified: "03/07/2019",
    isActive: true,
    chartData: [28, 30, 32, 31, 29, 28, 30, 31, 32, 30],
  },
];

interface Alert {
  id: string;
  dateTime: string;
  type: "disconnect" | "tempUp" | "tempDown";
  device: string;
  description: string;
}

const mockAlerts: Alert[] = [
  {
    id: "1",
    dateTime: "10/11/2025 14:23:15",
    type: "disconnect",
    device: "Zona A - Almacén",
    description: "Sin conexión",
  },
  {
    id: "2",
    dateTime: "10/11/2025 15:45:32",
    type: "tempUp",
    device: "Zona B - Producción",
    description: "Temp. alta 28°C",
  },
  {
    id: "3",
    dateTime: "10/11/2025 16:12:08",
    type: "tempDown",
    device: "Zona C - Laboratorio",
    description: "Temp. baja -85°C",
  },
  {
    id: "4",
    dateTime: "09/11/2025 17:30:41",
    type: "disconnect",
    device: "Zona D - Oficinas",
    description: "Falla de red",
  },
  {
    id: "5",
    dateTime: "09/11/2025 18:05:19",
    type: "tempUp",
    device: "Zona E - Exterior",
    description: "Sobrecalentami.",
  },
];

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
  const [alerts, setAlerts] = useState<Alert[]>(mockAlerts);
  const [currentTime, setCurrentTime] = useState(new Date());
  const [isInternetConnected, setIsInternetConnected] = useState(true);
  const [isServerConnected, setIsServerConnected] = useState(true);
  const [isMuted, setIsMuted] = useState(false);
  const [isDarkMode, setIsDarkMode] = useState(false);
  const [showAbout, setShowAbout] = useState(false);

  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentTime(new Date());
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleDeleteAlert = (id: string) => {
    setAlerts(alerts.filter((alert) => alert.id !== id));
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
            Proyecto HMI
          </span>
        </div>

        <div className="flex justify-center">
          <span className="text-base font-medium text-white/90">
            {formatDateTime(currentTime)}
          </span>
        </div>

        <div className="flex items-center justify-end gap-6">
          <div className="flex items-center gap-4">
            <button
              onClick={() => setIsInternetConnected(!isInternetConnected)}
              className="group transition-all"
              title={
                isInternetConnected
                  ? "Internet conectado"
                  : "Internet desconectado"
              }
            >
              <Wifi className="h-5 w-5 text-white" />
            </button>

            <button
              onClick={() => setIsServerConnected(!isServerConnected)}
              className="group transition-all"
              title={
                isServerConnected
                  ? "Servidor conectado"
                  : "Servidor desconectado"
              }
            >
              <Server className="h-5 w-5 text-white" />
            </button>

            <button
              onClick={() => setIsDarkMode(!isDarkMode)}
              className="group transition-all"
              title={
                isDarkMode ? "Cambiar a modo claro" : "Cambiar a modo oscuro"
              }
            >
              {isDarkMode ? (
                <Sun className="h-5 w-5 text-white" />
              ) : (
                <Moon className="h-5 w-5 text-white" />
              )}
            </button>
          </div>

          <button
            onClick={() => setIsMuted(!isMuted)}
            className="text-white/90 transition-all hover:text-white hover:scale-110 active:scale-95"
            title={isMuted ? "Activar sonido" : "Silenciar"}
          >
            {isMuted ? (
              <VolumeX className="h-6 w-6" />
            ) : (
              <Volume2 className="h-6 w-6" />
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
                <span className="font-semibold">Versión:</span> 1.0.0
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

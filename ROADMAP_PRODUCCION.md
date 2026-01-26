# 🚀 Roadmap Producción - NXT-HMI

**Última actualización**: 21 de enero de 2026  
**Estado actual**: v0.1.0 - En transición a producción

---

## 📊 ANÁLISIS DEL PROYECTO

### ✅ Lo que está bien
- ✅ Backend Rust estable con gestión de alertas completa
- ✅ Dos fuentes de datos integradas (MQTT + Supabase Realtime)
- ✅ Control de hardware (buzzer) funcional
- ✅ Sistema de mute y silenciamiento de alertas
- ✅ Frontend React/Vite con componentes UI completos
- ✅ Manejo de errores exhaustivo con logs
- ✅ Graceful shutdown mejorado
- ✅ Sincronización thread-safe

### ⚠️ Problemas identificados
- ❌ Ruta de configuración hard-coded (`src-tauri/config/config.yaml`)
- ❌ README.md genérico sin documentación del proyecto
- ❌ Versión v0.1.0 debe ser actualizada
- ❌ Falta metadatos de la aplicación (Cargo.toml)
- ❌ Carpetas duplicadas (src/ vs front_nextjs/)
- ❌ Sin script de build/release automatizado
- ❌ Sin archivo .env.example actualizado
- ❌ Sin tests unitarios
- ❌ Sin CI/CD pipeline
- ❌ Certificados SSL/TLS en repositorio (seguridad)
- ❌ Sin versionado automático
- ❌ Sin documentación de deployment

### ⏳ Lo que falta
- Frontend incompleto (componentes existentes pero sin lógica de conexión)
- Persistencia de alertas en disco
- Panel de administración/configuración
- Exportación de logs
- Estadísticas/reportes
- Multi-idioma
- Temas/personalización

---

## 🎯 PRIORIDADES PARA PRODUCCIÓN

### 🔴 CRÍTICO (Semana 1)

#### 1. **Seguridad - Remover credenciales del repositorio**
- [ ] Remover archivo `src-tauri/config/config.yaml` del repo
- [ ] Crear `.env.example` con placeholders
- [ ] Crear `config.yaml.example` con valores por defecto
- [ ] Implementar lectura de variables de entorno
- [ ] Agregar `config.yaml` a `.gitignore`
- [ ] Remover certificados hardcoded si existen
- **Esfuerzo**: 2-3 horas
- **Bloqueante**: SÍ

#### 2. **Metadatos del proyecto**
- [ ] Actualizar `package.json` (version, description, author, license, repository)
- [ ] Actualizar `src-tauri/Cargo.toml` (authors, description, license, metadata)
- [ ] Crear `tauri.conf.json` versión producción
- [ ] Definir versión estable (0.1.0 o 1.0.0)
- [ ] Agregar `CHANGELOG.md`
- **Esfuerzo**: 1-2 horas
- **Bloqueante**: NO

#### 3. **Configuración de rutas**
- [ ] Implementar variable de entorno `CONFIG_PATH`
- [ ] Permitir configuración desde variable de entorno o archivo
- [ ] Crear `load_config_from_env()` en Rust
- [ ] Validar que la configuración se carga correctamente en producción
- **Esfuerzo**: 1 hora
- **Bloqueante**: SÍ

#### 4. **README.md profesional**
- [ ] Escribir descripción clara del proyecto
- [ ] Documentar requisitos del sistema
- [ ] Guía de instalación paso a paso
- [ ] Configuración inicial
- [ ] Troubleshooting común
- [ ] Links a documentación técnica
- **Esfuerzo**: 2-3 horas
- **Bloqueante**: NO (pero importante para usuarios)

#### 5. **Testing básico**
- [ ] Test unitarios para funciones críticas (Rust)
- [ ] Test de integración MQTT
- [ ] Test de integración Supabase
- [ ] Test de parsing de alertas
- **Esfuerzo**: 4-6 horas
- **Bloqueante**: SÍ

---

### 🟠 ALTO (Semana 2)

#### 6. **Build y Release automatizado**
- [ ] Crear GitHub Actions workflow para build
- [ ] Crear script de versioning automático
- [ ] Generar binarios para Windows/Linux
- [ ] Crear releases automáticas en GitHub
- [ ] Documentar proceso de deployment
- **Esfuerzo**: 3-4 horas
- **Bloqueante**: NO (pero necesario para deployment)

#### 7. **Gestión de logs mejorada**
- [ ] Implementar rotación de logs
- [ ] Guardar logs en archivo (no solo console)
- [ ] Configurar niveles de log por módulo
- [ ] Implementar timestamp consistente
- [ ] Exportar logs a archivo en caso de error
- **Esfuerzo**: 2-3 horas
- **Bloqueante**: NO

#### 8. **Frontend - Conectar con backend**
- [ ] Implementar comunicación con Tauri commands
- [ ] Listeners para eventos de alertas en tiempo real
- [ ] UI responsiva para diferentes resoluciones
- [ ] Manejo de desconexiones
- [ ] Indicadores visuales de estado
- **Esfuerzo**: 5-6 horas
- **Bloqueante**: SÍ

#### 9. **Persistencia de alertas**
- [ ] Guardar alertas en SQLite local
- [ ] Recuperar alertas al iniciar
- [ ] Limpiar alertas antiguas (> 30 días)
- [ ] Exportar histórico de alertas
- **Esfuerzo**: 3-4 horas
- **Bloqueante**: NO

#### 10. **Monitoreo y health checks**
- [ ] Endpoint de health check
- [ ] Indicadores de estado de conexiones
- [ ] Logs de reconexiones
- [ ] Alertas de fallo crítico
- **Esfuerzo**: 2-3 horas
- **Bloqueante**: NO

---

### 🟡 MEDIO (Semana 3-4)

#### 11. **Panel de configuración**
- [ ] Interfaz para editar config (MQTT, Supabase, buzzer)
- [ ] Validación de configuración
- [ ] Prueba de conexión a servidores
- [ ] Reinicio de servicios desde UI
- **Esfuerzo**: 4-5 horas
- **Bloqueante**: NO

#### 12. **Documentación técnica**
- [ ] API de Tauri commands documentada
- [ ] Arquitectura del proyecto explicada
- [ ] Guía de desarrollo
- [ ] Diagrama de flujo de datos
- **Esfuerzo**: 3-4 horas
- **Bloqueante**: NO

#### 13. **Mejoras de performance**
- [ ] Optimizar renderizado de alertas (virtualización)
- [ ] Lazy loading de componentes
- [ ] Reducir tamaño del bundle
- [ ] Caché mejorada en frontend
- **Esfuerzo**: 3-4 horas
- **Bloqueante**: NO

#### 14. **Manejo de errores mejorado**
- [ ] Try-catch globales en frontend
- [ ] Error boundaries en React
- [ ] Notificaciones de error al usuario
- [ ] Recuperación automática de fallos
- **Esfuerzo**: 2-3 horas
- **Bloqueante**: NO

---

### 🔵 BAJO (Futuro)

#### 15. **Multi-idioma (i18n)**
- [ ] Estructura de traducciones
- [ ] Implementar en UI
- [ ] Selecciona de idioma
- **Esfuerzo**: 4-5 horas

#### 16. **Panel de administración avanzado**
- [ ] Estadísticas de alertas
- [ ] Gráficos históricos
- [ ] Reportes descargables
- [ ] Auditoría de eventos
- **Esfuerzo**: 6-8 horas

#### 17. **Notificaciones push**
- [ ] Integración con sistema de notificaciones del SO
- [ ] Email de alertas críticas
- [ ] SMS de alertas críticas (opcional)
- **Esfuerzo**: 4-6 horas

#### 18. **Tema oscuro/claro**
- [ ] Sistema de temas en frontend
- [ ] Persistencia de preferencia
- [ ] Transiciones suaves
- **Esfuerzo**: 2 horas

#### 19. **Tests end-to-end**
- [ ] Automatización completa de escenarios
- [ ] Tests de carga
- [ ] Tests de estrés
- **Esfuerzo**: 5-6 horas

---

## 📋 CHECKLIST ANTES DE PRODUCCIÓN

### Seguridad
- [ ] Credenciales removidas de git
- [ ] No hay secrets en código
- [ ] SSL/TLS habilitado para MQTT
- [ ] Validación de entrada en todas partes
- [ ] Rate limiting implementado

### Estabilidad
- [ ] Tests unitarios pasando
- [ ] Tests de integración pasando
- [ ] Sin memory leaks (valgrind/profiling)
- [ ] Sin warnings en compilación
- [ ] Graceful shutdown funciona

### Documentación
- [ ] README.md completo
- [ ] CHANGELOG.md actualizado
- [ ] Guía de instalación
- [ ] Guía de troubleshooting
- [ ] Documentación de API

### Configuración
- [ ] Config path configurable
- [ ] Variables de entorno funcionales
- [ ] Valores por defecto seguros
- [ ] Validación de config al iniciar

### Deployment
- [ ] Build automatizado funciona
- [ ] Binarios generados correctamente
- [ ] Versioning automático funciona
- [ ] Instrucciones de instalación claras

---

## 📅 TIMELINE ESTIMADO

| Fase | Tareas | Duración | Fecha Estimada |
|------|--------|----------|-----------------|
| **CRÍTICO** | 1-5 | 13-17 horas | 22-23 Ene |
| **ALTO** | 6-10 | 17-21 horas | 24-27 Ene |
| **MEDIO** | 11-14 | 12-16 horas | 28-31 Ene |
| **BAJO** | 15-19 | 21-30 horas | Feb+ |

**Total CRÍTICO**: ~2-3 días  
**Total PRODUCCIÓN LISTA**: ~5-7 días  
**Total COMPLETO**: ~2-3 semanas

---

## 🚨 RIESGOS IDENTIFICADOS

| Riesgo | Impacto | Probabilidad | Mitigación |
|--------|---------|--------------|-----------|
| Pérdida de alertas | CRÍTICO | MEDIA | Implementar persistencia BD |
| Credenciales expuestas | CRÍTICO | ALTA | Remover immediatamente |
| Fallo de conexión MQTT | ALTO | MEDIA | Tests de reconexión |
| Fallo de Supabase | ALTO | BAJA | Fallback a MQTT |
| Memory leak en buzzer | ALTO | BAJA | Profiling antes de release |
| UI no responsiva | MEDIO | MEDIA | Testing en múltiples pantallas |
| Logs muy grandes | BAJO | MEDIA | Implementar rotación |

---

## 📞 NOTAS IMPORTANTES

1. **Credenciales**: Las actuales están en `src-tauri/src-tauri/config/config.yaml`. REMOVER antes de push a repo público.

2. **Rutas**: CONFIG_PATH está hard-coded. Hacer configurable via variable de entorno.

3. **Frontend**: Hay dos carpetas (src/ y front_nextjs/). Usar solo una en producción.

4. **Versioning**: Actualizar versión en:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `tauri.conf.json`

5. **Testing**: SIN tests unitarios actualmente. Implementar antes de Producción.

6. **CI/CD**: NO hay pipeline. Necesario para automated deployment.

---

## 👥 Responsables

- **Backend (Rust)**: Integración MQTT/Supabase, gestión de alertas
- **Frontend (React)**: UI, listeners de eventos, visualización
- **DevOps**: Build, tests, deployment
- **QA**: Validación completa en staging

---

**Generado automáticamente**: 21-01-2026  
**Siguiente revisión**: Después de completar CRÍTICO

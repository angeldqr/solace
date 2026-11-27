# Changelog

Todas las versiones publicadas de Solace. Las notas completas de cada una están en [GitHub Releases](https://github.com/angeldqr/solace/releases).

## 0.0.17 — 2026-11-27

**Novedades**

- **Nuevo logo:** Solace estrena ícono, un sol con el prompt de la terminal. Lo verás en la barra de tareas, el instalador, la barra lateral y Discord.

**Nota:** Windows puede seguir mostrando el ícono anterior en la barra de tareas hasta que reinicies el equipo.

## 0.0.16 — 2026-11-27

**Novedades**

- **Actividad en Discord activada por defecto:** con Discord abierto, tus amigos ven qué agente usas, por ejemplo "Programando con Claude Code", con el ícono de Solace y el del agente. Nunca se envían nombres de carpetas ni proyectos. Puedes desactivarla en _Ajustes > General > Discord_.
- **Buscar actualizaciones:** en _Ajustes > Acerca de_ hay un botón para buscar una versión nueva al momento.

**Consejo:** si Discord muestra solo "Jugando Solace" sin detalles, quita Solace de _Ajustes de Discord > Juegos registrados_.

## 0.0.15 — 2026-11-27

**Novedades**

- **Estado de Discord en Ajustes:** debajo de _Mostrar actividad en Discord_ ves si la actividad está visible, si Discord no está abierto o si la rechazó.

**Consejo:** si Discord muestra solo "Jugando Solace" sin detalles, quita Solace de _Ajustes de Discord > Juegos registrados_.

## 0.0.14 — 2026-11-27

**Novedades**

- **Actividad en Discord:** activa _Ajustes > General > Mostrar actividad en Discord_ y tus amigos verán qué agente usas, por ejemplo "Programando con Claude Code", con el ícono de Solace y el del agente. Nunca se envían nombres de carpetas ni de proyectos. Está desactivado por defecto y necesita Discord abierto.

## 0.0.13 — 2026-11-27

Solace ahora reparte trabajo en paralelo, muestra tu uso de Claude y Codex, y abre archivos sin salir de la app.

**Novedades**

- **Tareas en paralelo (Solace Tasks):** dile a Claude Code "tengo estas 4 tareas" y las reparte entre varios agentes. Cada uno trabaja en su propio git worktree y rama, revisa primero las skills que tienes y te sugiere otras de skills.sh sin instalarlas. Al terminar hace commit y deja un resumen. Un tablero en la barra lateral muestra el progreso. También funciona desde Codex y otros CLIs.
- **Medidor de uso:** junto a Files y Changes ves tu uso de Claude (5 horas y semanal) con cuenta atrás hasta el reinicio, los límites de Codex y los tokens de hoy. Requiere la integración con Claude Code activada (Ajustes > Agentes), que ahora también envuelve tu status line sin perderla.
- **Visor de archivos:** un clic en un archivo del panel Files lo abre ahí mismo. Markdown renderizado, código con resaltado y números de línea, e imágenes.
- **Personalización:** color de acento, fuentes de interfaz y de código, editor de tema con exportar e importar, CSS propio (con confirmación y el atajo Ctrl+Alt+Shift+Backspace para deshacerlo) y velocidad de animación.
- **Página Diffs en Ajustes:** vista unificada o dividida, colores rojo/verde o azul/naranja, ajuste de línea e ignorar espacios.
- **Interfaz:** el panel derecho ya no repite pestañas, la sección Agentes empieza a media altura y los espacios usan un punto de color en vez de la letra.

## 0.0.12 — 2026-11-26

Solace estrena una interfaz renovada.

**Novedades**

- **Temas claros y oscuros:** cada tema (Solace, Graphite, Grove y Ember) tiene ahora variante clara y oscura, y puedes elegir el modo Sistema, Claro u Oscuro. Si usabas Daylight, pasas a Solace en modo claro.
- **Ajustes rediseñados:** navegación por secciones, búsqueda (tecla `/`), vista previa de los temas y botón para restaurar los valores por defecto. Nuevos controles para el tamaño de la interfaz, el contraste y la opacidad del vidrio.
- **Panel de Archivos y Cambios más amplio:** pestañas propias, íconos a color por tipo de archivo, colores según el estado de git y un botón para ampliar el panel.
- **Diffs con resaltado de sintaxis:** números de línea y vista unificada o dividida.
- **Barra lateral redimensionable**, pestañas más claras y una paleta de comandos agrupada con íconos y atajos.
- Tipografía algo más grande y consistente en toda la app.

## 0.0.11 — 2026-11-26

Solace llega a macOS y Linux.

**Novedades**

- **macOS (Apple Silicon) y Linux:** descarga el `.dmg` para Mac, o el `.AppImage` o el `.deb` para Linux. Es la primera versión para estas plataformas: si algo falla, abre un issue con tu sistema.
- **Reiniciar un panel:** cuando el shell termina, el panel lo indica y Enter abre uno nuevo en el mismo sitio.
- **Divisiones con el teclado:** Alt+Shift+flechas mueve la división más cercana.
- **Archivos renombrados:** el panel Cambios muestra lo que cambió respecto al original, en vez del archivo entero como nuevo.

**Seguridad**

- Abrir el panel Cambios en un repositorio descargado ya no ejecuta los filtros de git que ese repositorio define.

**Notas**

- En macOS la app no está notarizada: la primera vez ábrela con clic derecho > Abrir, o ejecuta `xattr -dr com.apple.quarantine /Applications/Solace.app`.
- El `.deb` no se actualiza solo; el AppImage, el Mac y Windows sí.
- Los atajos usan Ctrl también en macOS.

## 0.0.10 — 2026-11-26

Agentes también dentro de WSL.

**Novedades**

- **Detección en WSL (experimental):** si abres WSL en un panel y lanzas Claude Code, Codex, Gemini u otro agente dentro de Linux, Solace ahora lo reconoce y muestra su estado igual que en Windows, también los instalados con npm.
- Comandos como `wsl --shutdown` o `wsl -l` ya no se confunden con una sesión.

Si algo no se detecta bien dentro de WSL, abre un issue con la distribución y el agente que usas.

## 0.0.9 — 2026-11-26

Terminal más fiel.

**Novedades**

- **ConPTY actualizada:** Solace incluye la ConPTY 1.24 de Microsoft, la capa que conecta la terminal con la shell, en lugar de usar la versión más antigua que trae Windows. Menos fallos al dibujar y mejor soporte de las aplicaciones de terminal modernas.
- Los agentes que Solace abre sin mostrar arrancan al instante.
- **Seguridad:** Solace ya no carga bibliotecas desde el `PATH`.

Si notas algún problema en la terminal, puedes volver a la ConPTY de Windows definiendo la variable de entorno `SOLACE_WINDOWS_CONPTY=system`.

## 0.0.8 — 2026-11-26

Integración con Claude Code.

**Novedades**

- **Estado exacto de Claude Code:** activa _Ajustes › Agentes › Integración con Claude Code_ y Claude le dirá a Solace cuándo trabaja, cuándo te necesita y cuándo termina, en lugar de que Solace lo deduzca de la pantalla. Los avisos llegan antes y sin falsos positivos.
- La integración también instala el skill que enseña a Claude a usar el comando `solace` para abrir paneles y coordinar otros agentes.
- Tus hooks y ajustes de Claude Code se conservan, y antes del primer cambio se guarda una copia de `settings.json`. Fuera de Solace, los hooks no hacen nada.

## 0.0.7 — 2026-11-25

Notificaciones de agentes y preparación de la firma de código.

**Novedades**

- **Notificaciones:** Solace te avisa cuando un agente que no estás mirando te necesita (un permiso o una pregunta) o termina una tarea. Si te necesita, el botón de la barra de tareas parpadea hasta que vuelvas. Funciona aunque Solace esté minimizado, y se activa o desactiva en Ajustes.
- **Privacidad:** en Ajustes › Acerca de puedes desactivar la búsqueda de actualizaciones, la única conexión que Solace hace por su cuenta.
- El CLI `solace` muestra producto y versión en sus propiedades de Windows.

## 0.0.6 — release no

## 0.0.5 — 2026-11-25

Ajustes, temas y personalización.

**Novedades**

- **Ajustes** (Ctrl+, o el botón _Settings_ abajo a la izquierda).
- **Temas:** Solace, Graphite, Grove, Ember y Daylight (claro). Se aplican al instante a la interfaz y a todas las terminales.
- **Importar temas de VS Code:** pega el JSON del tema y listo.
- **Terminal:** fuente (incluida una propia), tamaño, interlineado y estilo de cursor.
- **Shell para terminales nuevas:** PowerShell, cmd, Git Bash o WSL.
- **Idioma:** automático, español o inglés.

## 0.0.4 — 2026-11-25

Paneles de archivos y cambios, y la primera actualización automática.

**Novedades**

- **Panel Archivos** (Ctrl+Shift+B o el botón _Files_): árbol del proyecto que carga cada carpeta al abrirla, con lo ignorado por git atenuado. Doble clic abre documentos y código; los ejecutables solo se muestran en el Explorador.
- **Panel Cambios** (Ctrl+Shift+G o el botón _Changes_): archivos modificados con líneas añadidas y quitadas, conflictos marcados y vista del diff con colores.
- Ambos se actualizan solos al guardar archivos, hacer commit o cambiar de rama.
- git se ejecuta sin confiar en la configuración del repositorio abierto, para que una carpeta descargada no pueda ejecutar programas al abrirla.

## 0.0.3 — 2026-11-25

Solace ya entiende a tus agentes y les permite trabajar en equipo.

**Novedades**

- **Detección de agentes:** reconoce Claude Code, Codex, Gemini CLI, Qwen Code, Kimi Code, OpenCode, Copilot, Cursor Agent y Amp en cada panel, y muestra si están trabajando, si te necesitan (un permiso o una pregunta) o si terminaron mientras mirabas otra cosa.
- **Orquestación:** desde cualquier terminal de Solace, el comando `solace` permite que un agente abra paneles, lance otros agentes, les envíe tareas y lea sus respuestas (`solace agent start`, `solace agent prompt --wait`, …). Instala la guía para Claude Code con `solace skill install`.
- **Actualizaciones automáticas:** a partir de esta versión Solace descarga las nuevas versiones en segundo plano y te ofrece "Reiniciar para actualizar" en la barra de estado. Nunca reinicia por su cuenta.
- Más robusto: recargar la interfaz conserva la pantalla de los agentes y Ctrl+C funciona siempre.

**Instalación:** descarga `Solace_0.0.3_x64-setup.exe`. Si tienes la 0.0.2, instala esta encima una vez; desde aquí las actualizaciones llegan solas. El instalador aún no está firmado con certificado, así que SmartScreen puede pedir confirmación ("Más información" → "Ejecutar de todas formas").

## 0.0.2 — 2026-11-25

Primera versión usable de Solace como terminal.

**Novedades**

- Terminales reales (PowerShell) sobre ConPTY con xterm.js y renderizado WebGL.
- Espacios por carpeta, con la rama git visible.
- Pestañas y divisiones redimensionables; la sesión se guarda y se restaura al reabrir.
- Paleta de comandos (Ctrl+Shift+P) y atajos: Ctrl+Shift+O abrir carpeta, Ctrl+Shift+T nueva terminal, Ctrl+Shift+D / Ctrl+Shift+E dividir, Ctrl+Shift+W cerrar.
- Interfaz en español e inglés según el idioma del sistema.

**Instalación:** descarga `Solace_0.0.2_x64-setup.exe` (recomendado) o el `.msi`. El instalador aún no está firmado, así que Windows SmartScreen puede pedir confirmación ("Más información" → "Ejecutar de todas formas").

Versión temprana: todavía no detecta agentes (llega en la próxima versión).

## 0.0.1 — Download `

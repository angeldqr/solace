const en = {
  spaces: "Spaces",
  openFolder: "Open folder",
  noSpaces: "No spaces yet",
  agents: "Agents",
  agentsEmpty: "Agents you start will appear here",
  startTitle: "Start with a folder",
  startBody: "Each folder becomes a space with its own terminals and agents.",
  noTerminals: "No terminals open",
  newTerminal: "New terminal",
  splitRight: "Split right",
  splitDown: "Split down",
  closeTerminal: "Close terminal",
  closeSpace: "Close space",
  closeTab: "Close tab",
  nextTab: "Next tab",
  previousTab: "Previous tab",
  commands: "Commands",
  searchCommands: "Search commands",
  noMatches: "No matching commands",
  thisComputer: "This computer",
  noRepository: "No repository",
  agentCount: "{n} agents",
  minimize: "Minimize",
  maximize: "Maximize",
  close: "Close",
  terminal: "Terminal",
};

type Messages = typeof en;
export type MessageKey = keyof Messages;

const es: Messages = {
  spaces: "Espacios",
  openFolder: "Abrir carpeta",
  noSpaces: "Aún no hay espacios",
  agents: "Agentes",
  agentsEmpty: "Los agentes que inicies aparecerán aquí",
  startTitle: "Empieza con una carpeta",
  startBody: "Cada carpeta se convierte en un espacio con sus propias terminales y agentes.",
  noTerminals: "No hay terminales abiertas",
  newTerminal: "Nueva terminal",
  splitRight: "Dividir a la derecha",
  splitDown: "Dividir hacia abajo",
  closeTerminal: "Cerrar terminal",
  closeSpace: "Cerrar espacio",
  closeTab: "Cerrar pestaña",
  nextTab: "Pestaña siguiente",
  previousTab: "Pestaña anterior",
  commands: "Comandos",
  searchCommands: "Buscar comandos",
  noMatches: "Ningún comando coincide",
  thisComputer: "Este equipo",
  noRepository: "Sin repositorio",
  agentCount: "{n} agentes",
  minimize: "Minimizar",
  maximize: "Maximizar",
  close: "Cerrar",
  terminal: "Terminal",
};

export const locale = navigator.language.toLowerCase().startsWith("es") ? "es" : "en";
const messages = locale === "es" ? es : en;

export function t(key: MessageKey, vars: Record<string, string | number> = {}) {
  return messages[key].replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? ""));
}

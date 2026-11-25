/** True when `event` matches a combo like "Ctrl+Shift+T". */
export function matches(event: KeyboardEvent, combo: string) {
  const parts = combo.split("+");
  const key = parts[parts.length - 1].toLowerCase();
  return (
    event.ctrlKey === parts.includes("Ctrl") &&
    event.shiftKey === parts.includes("Shift") &&
    event.altKey === parts.includes("Alt") &&
    event.key.toLowerCase() === key
  );
}

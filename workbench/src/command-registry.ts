/**
 * Semantic command layer (ICM §4 command envelope, §34 binding layer, §48
 * registry validation). Keys are ONE representation of intent; pointer
 * gestures, menus, and palette rows resolve to the same SemanticCommandId
 * through this registry. The shell's reducer dispatches on command identity,
 * never on physical keys, so mouse and keyboard cannot fork behavior.
 *
 * Invariants:
 * - Command ids are stable semantic identity (ICM §4.1): renaming one breaks
 *   equivalence tests on purpose.
 * - A key binding exists only in a KeySurface context; the same physical key
 *   may mean different commands in different contexts (palette owns input).
 * - `validateCommandRegistry` is the startup check (ICM §48): ids unique,
 *   default keys unique per context, every bound key resolvable.
 */

export type SemanticCommandId =
  // Application
  | "app.quit"
  // View activation (primary views)
  | "view.activate.work"
  | "view.activate.chat"
  | "view.activate.trace"
  // Surface/presentation navigation
  | "ui.surface.next"
  | "ui.surface.prev"
  | "ui.dismiss"
  | "ui.composer.focus"
  // Bottom surface / inspector presentation
  | "ui.bottom.toggle"
  | "ui.inspector.toggle"
  | "ui.inspector.pin.toggle"
  // Palette
  | "ui.palette.open"
  | "ui.palette.close"
  | "ui.palette.dismiss"
  | "ui.palette.move.prev"
  | "ui.palette.move.next"
  | "ui.palette.activate"
  // Movement/selection
  | "nav.move.prev"
  | "nav.move.next"
  | "nav.focus.select"
  // Trace stream
  | "trace.follow.resume"
  // Horizon
  | "nav.horizon.hoist"
  | "nav.horizon.dehoist";

/** Command classes follow ICM §5 (presentation-only → navigation → query → runtime → canonical). */
export type SemanticCommandClass = "PRESENTATION_LOCAL" | "NAVIGATION_LOCAL" | "DERIVED_QUERY";

/** Input contexts that rebind keys. Mirrors the shell's input precedence:
 * below-minimum prompt and open palette own the keyboard before the shell. */
export type CommandKeySurface = "below_minimum" | "palette_open" | "shell";

export interface CommandKeyContext {
  readonly surface: CommandKeySurface;
}

export interface SemanticCommandSpec {
  readonly id: SemanticCommandId;
  readonly title: string;
  readonly commandClass: SemanticCommandClass;
  /** Key surfaces this command is reachable from. */
  readonly surfaces: readonly CommandKeySurface[];
  /** Default physical key in each surface (ICM §34.1 earned bindings). */
  readonly defaultKeys: Readonly<Partial<Record<CommandKeySurface, string>>>;
}

/** The commands the shell implements today, with their earned bindings. */
export const COMMAND_REGISTRY: readonly SemanticCommandSpec[] = Object.freeze([
  {
    id: "app.quit",
    title: "Quit workbench",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["below_minimum", "shell"],
    defaultKeys: { below_minimum: "q", shell: "q" },
  },
  {
    id: "view.activate.work",
    title: "Open Work view",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "1" },
  },
  {
    id: "view.activate.chat",
    title: "Open Chat view",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "2" },
  },
  {
    id: "view.activate.trace",
    title: "Open Trace view",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "3" },
  },
  {
    id: "ui.surface.next",
    title: "Focus next surface",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "tab" },
  },
  {
    id: "ui.surface.prev",
    title: "Focus previous surface",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "shift+tab" },
  },
  {
    id: "ui.dismiss",
    title: "Dismiss to Primary",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "escape" },
  },
  {
    id: "ui.composer.focus",
    title: "Focus composer",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "e" },
  },
  {
    id: "trace.follow.resume",
    title: "Return trace to live",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "g" },
  },
  {
    id: "ui.bottom.toggle",
    title: "Toggle bottom surface",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "b" },
  },
  {
    id: "ui.inspector.toggle",
    title: "Toggle inspector drawer",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "i" },
  },
  {
    id: "ui.inspector.pin.toggle",
    title: "Pin/unpin inspector target",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "I" },
  },
  {
    id: "ui.palette.open",
    title: "Open command palette",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "p" },
  },
  {
    id: "ui.palette.close",
    title: "Close palette",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["palette_open"],
    defaultKeys: { palette_open: "p" },
  },
  {
    id: "ui.palette.dismiss",
    title: "Dismiss palette",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["palette_open"],
    defaultKeys: { palette_open: "escape" },
  },
  {
    id: "ui.palette.move.prev",
    title: "Palette selection up",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["palette_open"],
    defaultKeys: { palette_open: "up" },
  },
  {
    id: "ui.palette.move.next",
    title: "Palette selection down",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["palette_open"],
    defaultKeys: { palette_open: "down" },
  },
  {
    id: "ui.palette.activate",
    title: "Activate palette row",
    commandClass: "PRESENTATION_LOCAL",
    surfaces: ["palette_open"],
    defaultKeys: { palette_open: "enter" },
  },
  {
    id: "nav.move.prev",
    title: "Move selection up",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "up" },
  },
  {
    id: "nav.move.next",
    title: "Move selection down",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "down" },
  },
  {
    id: "nav.focus.select",
    title: "Select focused item",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "enter" },
  },
  {
    id: "nav.horizon.hoist",
    title: "Hoist horizon to focus",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "h" },
  },
  {
    id: "nav.horizon.dehoist",
    title: "Dehoist one level",
    commandClass: "NAVIGATION_LOCAL",
    surfaces: ["shell"],
    defaultKeys: { shell: "u" },
  },
] as const);

const BY_KEY: ReadonlyMap<string, SemanticCommandId> = buildKeyMap();

function keyFor(surface: CommandKeySurface, key: string): string {
  return `${surface}::${key}`;
}

function buildKeyMap(): ReadonlyMap<string, SemanticCommandId> {
  const map = new Map<string, SemanticCommandId>();
  for (const command of COMMAND_REGISTRY) {
    for (const surface of command.surfaces) {
      const key = command.defaultKeys[surface];
      if (key !== undefined) {
        map.set(keyFor(surface, key), command.id);
      }
    }
  }
  return map;
}

/** Resolve a physical key in a context to its semantic command, or null when
 * the key is unbound in that context (unbound = ignored, never guessed). */
export function commandForKey(key: string, context: CommandKeyContext): SemanticCommandId | null {
  return BY_KEY.get(keyFor(context.surface, key)) ?? null;
}

/** The default key for a command in the shell surface (help/palette display);
 * null when the command has no shell-surface default binding. */
export function commandShortcut(id: SemanticCommandId): string | null {
  const command = COMMAND_REGISTRY.find((entry) => entry.id === id);
  return command?.defaultKeys.shell ?? null;
}

/** Startup validation (ICM §48). Returns an empty array when the registry is
 * sound; each string names one violation for fail-loud startup. */
export function validateCommandRegistry(
  registry: readonly SemanticCommandSpec[] = COMMAND_REGISTRY,
): readonly string[] {
  const problems: string[] = [];
  const ids = new Set<SemanticCommandId>();
  for (const command of registry) {
    if (ids.has(command.id)) {
      problems.push(`duplicate command id: ${command.id}`);
    }
    ids.add(command.id);
    if (command.title.length === 0) {
      problems.push(`command without title: ${command.id}`);
    }
    for (const surface of command.surfaces) {
      if (command.defaultKeys[surface] === undefined) {
        problems.push(`command ${command.id} declares surface ${surface} without a default key`);
      }
    }
  }
  const keys = new Map<string, SemanticCommandId>();
  for (const command of registry) {
    for (const surface of command.surfaces) {
      const key = command.defaultKeys[surface];
      if (key === undefined) continue;
      const slot = keyFor(surface, key);
      const existing = keys.get(slot);
      if (existing !== undefined && existing !== command.id) {
        problems.push(`key ${key} bound to both ${existing} and ${command.id} in ${surface}`);
      }
      keys.set(slot, command.id);
    }
  }
  // Round trip: every bound key must resolve back to its command.
  for (const [slot, id] of keys) {
    const separator = slot.indexOf("::");
    const surface = slot.slice(0, separator) as CommandKeySurface;
    const key = slot.slice(separator + 2);
    if (commandForKey(key, { surface }) !== id) {
      problems.push(`key ${key} in ${surface} does not resolve to ${id}`);
    }
  }
  return problems;
}

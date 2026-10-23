import type { ConfigSettings, SettingSources, SettingsDocument } from "@/client/types.gen";
import type { Owner } from "@/domain/config/words";

/** What a section of the builder reads and changes: the settings of one part of the document. */
export interface Editor {
  canEdit: boolean;
  owner: Owner;
  /** The settings that follow, with their sources. */
  resolved: ConfigSettings;
  /** What the part sets in the draft, and in the file. */
  own: SettingsDocument;
  base: SettingsDocument;
  /** Sets keys of the part, in one change; `null` leaves a key to a weaker source. */
  set: (values: SettingsDocument) => void;
}

/** The part sets the key. */
export function isSet(editor: Editor, key: keyof SettingsDocument): boolean {
  return editor.own[key] !== undefined && editor.own[key] !== null;
}

/** The draft changed the key from the file. */
export function isChanged(editor: Editor, key: keyof SettingsDocument): boolean {
  return JSON.stringify(editor.own[key] ?? null) !== JSON.stringify(editor.base[key] ?? null);
}

/** The keys of the file behind a row of the builder, and the setting whose source the row shows. */
export interface RowKeys {
  keys: (keyof SettingsDocument)[];
  source: keyof SettingSources;
}

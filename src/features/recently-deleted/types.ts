export type DeletedItemType = "credential" | "note";

export type DeletedItem = {
  id: string;
  itemType: DeletedItemType;
  title: string;
  deletedAtMs: number;
  originalSource: "Vault" | "Notes";
};

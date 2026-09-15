export type NoteInput = {
  title: string;
  content: string;
  tags: string[];
  favorite: boolean;
};

export type Note = NoteInput & {
  id: string;
  createdAtMs: number;
  updatedAtMs: number;
};

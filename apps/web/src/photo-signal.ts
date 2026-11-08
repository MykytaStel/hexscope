/** The private, local clues used to find related photos in a batch. */
export interface PhotoSignal {
  serial: string | null;
  location: [number, number] | null;
  taken: string | null;
}

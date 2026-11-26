# PNG text coverage

Missing keyword separators and unknown compressed-text methods previously returned no finding with only a warning. Invalid iTXt flags were interpreted as compressed. Record an error for unread text metadata instead, preserving the chunk and fields that were read. This makes inspection partial and the existing PNG cleaner conservatively refuses these damaged inputs. The uncompressed iTXt method byte is still ignored. Tests cover all three text chunk types and invalid compression fields. No allocation limits change.

The photo view now separates visible image properties from metadata and
privacy findings. It shows parsed pixel dimensions, PNG pixel encoding, stored
EXIF orientation, colour-profile markers, frame counts and auxiliary-image
markers where available. A property links to its parser node when the tree
contains one. This view does not claim that auxiliary images or animation
frames were decoded. Clean-copy findings remain tied to the Core read-back
report; the existing before/after comparison only draws bounded browser
previews.

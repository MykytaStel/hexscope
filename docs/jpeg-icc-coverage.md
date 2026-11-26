# JPEG ICC coverage

The existing ICC reader reports missing, duplicate or conflicting fragments as warnings and returns without inspecting the profile. Return an explicit assembly status and propagate it through the existing JPEG metadata_complete field, including the fragment count cap and failed range assembly. Keep warnings and source map behavior unchanged. This is profile assembly coverage, not a claim of complete semantic ICC validation. The tests exercise missing and duplicate fragments through the ordinary JPEG parser and summary. Existing removal rules and limits remain unchanged.

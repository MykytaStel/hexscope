#!/usr/bin/env python3
"""Builds two SVG drawings as the programs that make most of them leave them.

- crates/hexscope-core/tests/fixtures/inkscape.svg, also the site's sample
  apps/web/public/samples/drawing.svg: as Inkscape 0.92 saves a drawing —
  the file's name, the whole path it was last exported to, a picture linked
  by its path on the author's computer, a photo embedded as base64 that
  keeps its GPS (the test photo, taken at the Eiffel Tower), the window's
  settings, and RDF metadata with the creator, title, date and keywords.
- crates/hexscope-core/tests/fixtures/illustrator.svg: as Illustrator saves
  one with its editing capabilities kept — a comment naming it, a DOCTYPE
  with its own entities, its private copy of the drawing (i:pgf) — with a
  script added, as a page that animates a drawing would.

Every name and path is invented.

    python3 scripts/make-sample-svg.py
"""
import base64
import os

ROOT = os.path.join(os.path.dirname(__file__), "..")
FIXTURES = os.path.join(ROOT, "crates", "hexscope-core", "tests", "fixtures")
SAMPLES = os.path.join(ROOT, "apps", "web", "public", "samples")

photo = base64.b64encode(open(os.path.join(FIXTURES, "photo.jpg"), "rb").read()).decode()

inkscape = f'''<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<!-- Created with Inkscape (http://www.inkscape.org/) -->

<svg
   xmlns:dc="http://purl.org/dc/elements/1.1/"
   xmlns:cc="http://creativecommons.org/ns#"
   xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
   xmlns:svg="http://www.w3.org/2000/svg"
   xmlns="http://www.w3.org/2000/svg"
   xmlns:xlink="http://www.w3.org/1999/xlink"
   xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
   xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
   width="210mm"
   height="148mm"
   viewBox="0 0 210 148"
   version="1.1"
   id="svg8"
   inkscape:version="0.92.5 (2060ec1f9f, 2020-04-08)"
   sodipodi:docname="poster-client-final-v3.svg"
   inkscape:export-filename="/home/olena.koval/Work/Acme/poster-final.png"
   inkscape:export-xdpi="300"
   inkscape:export-ydpi="300">
  <defs
     id="defs2" />
  <sodipodi:namedview
     id="base"
     pagecolor="#ffffff"
     bordercolor="#666666"
     inkscape:zoom="0.7"
     inkscape:cx="396.9"
     inkscape:cy="280.6"
     inkscape:document-units="mm"
     inkscape:current-layer="layer1"
     inkscape:window-width="1920"
     inkscape:window-height="1016"
     inkscape:window-x="0"
     inkscape:window-y="27"
     inkscape:window-maximized="1" />
  <metadata
     id="metadata5">
    <rdf:RDF>
      <cc:Work
         rdf:about="">
        <dc:format>image/svg+xml</dc:format>
        <dc:type
           rdf:resource="http://purl.org/dc/dcmitype/StillImage" />
        <dc:title>Acme autumn poster</dc:title>
        <dc:date>2026-09-14</dc:date>
        <dc:creator>
          <cc:Agent>
            <dc:title>Olena Koval</dc:title>
          </cc:Agent>
        </dc:creator>
        <dc:publisher>
          <cc:Agent>
            <dc:title>Acme Print Ltd</dc:title>
          </cc:Agent>
        </dc:publisher>
        <dc:description>Draft for the client, not approved yet</dc:description>
        <dc:subject>
          <rdf:Bag>
            <rdf:li>acme</rdf:li>
            <rdf:li>poster</rdf:li>
          </rdf:Bag>
        </dc:subject>
      </cc:Work>
    </rdf:RDF>
  </metadata>
  <g
     inkscape:label="Layer 1"
     inkscape:groupmode="layer"
     id="layer1">
    <rect
       style="fill:#1f6feb;stroke:none"
       id="rect10"
       width="190"
       height="60"
       x="10"
       y="10" />
    <text
       xml:space="preserve"
       style="font-size:12px;fill:#ffffff"
       x="20"
       y="45"
       id="text12"><tspan
         sodipodi:role="line"
         id="tspan14"
         x="20"
         y="45">Autumn sale</tspan></text>
    <image
       y="80"
       x="10"
       id="image16"
       xlink:href="file:///home/olena.koval/Pictures/acme-logo.png"
       sodipodi:absref="/home/olena.koval/Pictures/acme-logo.png"
       preserveAspectRatio="none"
       height="40"
       width="40" />
    <image
       y="80"
       x="60"
       id="image18"
       xlink:href="data:image/jpeg;base64,{photo}"
       preserveAspectRatio="none"
       height="60"
       width="90" />
  </g>
</svg>
'''

illustrator = '''<?xml version="1.0" encoding="utf-8"?>
<!-- Generator: Adobe Illustrator 27.5.0, SVG Export Plug-In . SVG Version: 6.00 Build 0)  -->
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd" [
	<!ENTITY ns_extend "http://ns.adobe.com/Extensibility/1.0/">
	<!ENTITY ns_ai "http://ns.adobe.com/AdobeIllustrator/10.0/">
	<!ENTITY ns_graphs "http://ns.adobe.com/Graphs/1.0/">
	<!ENTITY ns_vars "http://ns.adobe.com/Variables/1.0/">
	<!ENTITY ns_imrep "http://ns.adobe.com/ImageReplacement/1.0/">
	<!ENTITY ns_sfw "http://ns.adobe.com/SaveForWeb/1.0/">
	<!ENTITY ns_custom "http://ns.adobe.com/GenericCustomNamespace/1.0/">
	<!ENTITY ns_adobe_xpath "http://ns.adobe.com/XPath/1.0/">
]>
<svg version="1.1" id="Layer_1" xmlns:x="&ns_extend;" xmlns:i="&ns_ai;" xmlns:graph="&ns_graphs;"
	 xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px" viewBox="0 0 200 100"
	 style="enable-background:new 0 0 200 100;" xml:space="preserve">
<switch>
	<foreignObject requiredExtensions="&ns_ai;" x="0" y="0" width="1" height="1">
		<i:pgfRef  xlink:href="#adobe_illustrator_pgf">
		</i:pgfRef>
	</foreignObject>
	<g i:extraneous="self">
		<circle cx="50" cy="50" r="40" fill="#E30613" onclick="spin(this)"/>
		<text x="100" y="55">Badge</text>
	</g>
</switch>
<script type="text/javascript"><![CDATA[
	function spin(el) { el.style.transform = "rotate(20deg)"; }
]]></script>
<i:pgf  id="adobe_illustrator_pgf">
	<![CDATA[
	eJzsvWuTHDdyKPpdEfoPfT84wo64Q+OVeNgOR5BNcr26XkkhyVo5Ntex9JCSdcyZ8RmSOt7z629m
	Ah8AFKoQs82aRm93z7I0nLdHxlZVQqOfn3+M+PzHr4+fP9fb+18/+++eXd3e/vfb+/bfv/r57/9P
	]]>
</i:pgf>
</svg>
'''

for path, text in [
    (os.path.join(FIXTURES, "inkscape.svg"), inkscape),
    (os.path.join(SAMPLES, "drawing.svg"), inkscape),
    (os.path.join(FIXTURES, "illustrator.svg"), illustrator),
]:
    with open(path, "w", newline="\n") as f:
        f.write(text)
    print(path, len(text.encode()))

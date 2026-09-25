//! What each part of a WebAssembly module is. Sections are cited to the
//! WebAssembly Core Specification; custom sections to its appendix and to
//! the tool conventions the toolchains share.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const CUSTOM: &str = "https://webassembly.github.io/spec/core/appendix/custom.html#name-section";
const PRODUCERS: &str =
    "https://github.com/WebAssembly/tool-conventions/blob/main/ProducersSection.md";
const DEBUGGING: &str = "https://github.com/WebAssembly/tool-conventions/blob/main/Debugging.md";
const FEATURES: &str = "https://github.com/WebAssembly/tool-conventions/blob/main/Linking.md";
const DWARF: &str = "https://yurydelendik.github.io/webassembly-dwarf/";
const COMPONENT: &str =
    "https://github.com/WebAssembly/component-model/blob/main/design/mvp/Binary.md";

const fn spec(text: &'static str, anchor: &'static str) -> Doc {
    Doc::new(text).cite("WebAssembly Core §5.5", anchor)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const TABLE: Table = &[
    ("module", spec("A WebAssembly module: code a browser or another engine runs, with the types, imports and exports around it.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-module")),
    ("magic", spec("The four bytes every WebAssembly file starts with: a zero, then “asm”.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-magic")),
    ("version", spec("The binary format's version: 1 for every module engines run today.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-version")),
    // Sections.
    ("section · type", spec("The function types: the parameters and results of every function the module calls or defines.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-typesec")),
    ("section · import", spec("What the module needs from its host: functions, memory, tables and globals, by module and name.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-importsec")),
    ("section · function", spec("Which type each of the module's own functions has; their code is in the code section.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-funcsec")),
    ("section · table", spec("Tables: lists of references, used for calls through a pointer.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-tablesec")),
    ("section · memory", spec("The module's memory: how many 64 KB pages it starts with, and may grow to.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-memsec")),
    ("section · tag", Doc::new("Exception tags: the kinds of exception the module can throw.").cite("WebAssembly exception handling", "https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md")),
    ("section · global", spec("Global variables and their starting values.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-globalsec")),
    ("section · export", spec("What the module offers its host, by name: the functions a page can call.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-exportsec")),
    ("section · start", spec("A function that runs as soon as the module is loaded.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-startsec")),
    ("section · element", spec("Element segments: what the tables are filled with.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-elemsec")),
    ("section · data count", spec("How many data segments follow, so they can be checked before the code that uses them.", "https://webassembly.github.io/spec/core/binary/modules.html#data-count-section")),
    ("section · code", spec("The code: the body of every function the module defines, as compact instructions.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-codesec")),
    ("section · data", spec("Data segments: bytes copied into memory at start — text, tables, constants the code uses.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-datasec")),
    ("custom section · name", Doc::new("Names for the module's functions and locals, which engines use in stack traces; not needed to run.").cite("WebAssembly Core §7.4", CUSTOM)),
    ("custom section · producers", Doc::new("The languages and tools that made the module, with their versions; not needed to run.").cite("WebAssembly tool conventions", PRODUCERS)),
    ("custom section · sourceMappingURL", Doc::new("Where the module's source map is: a link from its code back to the source files.").cite("WebAssembly tool conventions", DEBUGGING)),
    ("custom section · external_debug_info", Doc::new("Where the module's debug info was moved to: a link to another file.").cite("WebAssembly tool conventions", DEBUGGING)),
    ("custom section · target_features", Doc::new("The WebAssembly features the module was built for, which linkers check.").cite("WebAssembly tool conventions", FEATURES)),
    ("custom section · .debug*", Doc::new("DWARF debug info: source files, lines and variable names, for debuggers; not needed to run.").cite("DWARF for WebAssembly", DWARF)),
    ("custom section · *", spec("A custom section: data for tools, which engines skip.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-customsec")),
    ("component section *", Doc::new("A section of a WebAssembly component, which this tool lists but does not decode.").cite("Component model binary format", COMPONENT)),
    ("section *", spec("A section with an id the specification does not define.", "https://webassembly.github.io/spec/core/binary/modules.html#sections")),
    // Fields.
    ("id", spec("Which section this is.", "https://webassembly.github.io/spec/core/binary/modules.html#sections")),
    ("size", spec("How many bytes the section's contents take.", "https://webassembly.github.io/spec/core/binary/modules.html#sections")),
    ("contents", spec("The section's contents, which this tool does not break down.", "https://webassembly.github.io/spec/core/binary/modules.html#sections")),
    ("count", spec("How many entries follow.", "https://webassembly.github.io/spec/core/binary/conventions.html#binary-list")),
    ("types in a newer form", Doc::new("Types in the form the garbage-collection proposal adds, which this tool does not decode.").cite("WebAssembly GC", "https://github.com/WebAssembly/gc/blob/main/proposals/gc/MVP.md")),
    ("type *", spec("A function type: its parameters, then its results.", "https://webassembly.github.io/spec/core/binary/types.html#binary-functype")),
    ("import *", spec("One thing the module needs from its host, by module and name, and what kind of thing it is.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-importsec")),
    ("type indices", spec("For each function the module defines, the number of its type.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-funcsec")),
    ("table *", spec("One table: what it holds, and its size.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-tablesec")),
    ("memory *", spec("One memory: its size in 64 KB pages, at the start and at most.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-memsec")),
    ("tag *", Doc::new("One exception tag, and the type of what it carries.").cite("WebAssembly exception handling", "https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md")),
    ("global *", spec("One global variable: its type, whether it can change, and its starting value.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-globalsec")),
    ("export *", spec("One thing the module offers, under this name.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-exportsec")),
    ("start function", spec("The function that runs on load.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-startsec")),
    ("segments", spec("The segments themselves, which this tool does not break down.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-elemsec")),
    ("function names", Doc::new("A name for each function, by its number.").cite("WebAssembly Core §7.4", CUSTOM)),
    ("function *", spec("One function's code: its local variables, then its instructions.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-code")),
    ("data segment *", spec("One data segment: where in memory it goes, and its bytes.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-data")),
    ("* more entries", Doc::new("Entries past the first 20,000, read but not listed one by one.")),
    ("name", spec("The custom section's name, which says what it holds.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-customsec")),
    ("module name", Doc::new("The module's own name.").cite("WebAssembly Core §7.4", CUSTOM)),
    ("local names", Doc::new("Names for each function's local variables.").cite("WebAssembly Core §7.4", CUSTOM)),
    ("other names", Doc::new("Names of other things: labels, types, tables, memories, globals or data.").cite("WebAssembly Core §7.4", CUSTOM)),
    ("language", Doc::new("The source languages the module was written in.").cite("WebAssembly tool conventions", PRODUCERS)),
    ("processed-by", Doc::new("The tools that compiled and processed the module, with their versions.").cite("WebAssembly tool conventions", PRODUCERS)),
    ("sdk", Doc::new("The SDK the module was built with.").cite("WebAssembly tool conventions", PRODUCERS)),
    ("URL", Doc::new("The link, as written by the tool that built the module.").cite("WebAssembly tool conventions", DEBUGGING)),
    ("feature", Doc::new("One feature: + used, - not allowed, = required.").cite("WebAssembly tool conventions", FEATURES)),
    ("DWARF data", Doc::new("The debug info itself, in DWARF's own format.").cite("DWARF for WebAssembly", DWARF)),
    ("payload", spec("The custom section's data, in a form only the tools that wrote it know.", "https://webassembly.github.io/spec/core/binary/modules.html#binary-customsec")),
    // Problems.
    ("the module header is cut short", damage("The file is too short to hold the magic bytes and version every module starts with.")),
    ("version is not 1*", Doc::new("A version no engine runs: the file is damaged, or from an experiment.").concern(Concern::Oddity)),
    ("a section runs past the end of the file", damage("A section claims more bytes than there are: the file was cut short.")),
    ("unknown section id *", damage("A section id the format does not define: engines refuse to load the module.")),
    ("the * section is out of order*", damage("Sections must come in a fixed order, each once: engines refuse this module.")),
    ("the section ends in the middle of an entry", damage("The section's bytes end, or break, in the middle of an entry.")),
    ("* bytes after the section's last entry", damage("The section is longer than its entries: engines refuse the module.")),
    ("the file ends in a section header", damage("The file stops in the middle of a section's id or size: it was cut short.")),
];

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    lookup(TABLE, &node.label, problem).or_else(|| {
        // A producers field of a kind the conventions do not list.
        let parent = node.parent.and_then(|p| tree.try_get(p))?;
        (!problem && parent.label == "custom section · producers").then_some(
            Doc::new("Tools the module was made with, under a heading of their own.")
                .cite("WebAssembly tool conventions", PRODUCERS),
        )
    })
}

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;

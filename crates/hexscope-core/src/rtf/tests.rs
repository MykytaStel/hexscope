use super::*;

const WORD: &[u8] = br#"{\rtf1\adeflang1025\ansi\ansicpg1251\uc1\deff0{\fonttbl{\f0\froman Times New Roman;}}{\colortbl;\red0\green0\blue0;}{\*\rsidtbl \rsid1245187\rsid2361521}{\*\generator Microsoft Word 16.0;}{\info{\title Lease \'f7\'e5\'f0\'ed\'e5\'f2\'ea\'e0}{\author \u1054\'ce\u1083\'eb\u1077\'e5\u1085\'ed\u1072\'e0 Koval}{\operator Petro Shevchuk}{\company Northwind}{\creatim\yr2026\mo9\dy14\hr9\min12}{\revtim\yr2026\mo10\dy2\hr18\min47}{\printim\yr2026\mo10\dy1\hr16\min3}{\version14}{\edmins192}{\nofpages2}}{\*\userprops {\propname Client}\proptype30{\staticval Northwind}}{\*\template C:\\Users\\olena\\Templates\\Lease.dotx}{\*\revtbl {Unknown;}{Olena Koval;}}
\pard Monthly rent: {\deleted\revauthdel1\revdttmdel1234 18 000 UAH; offer that first}{\revised\revauth1 20 000 UAH}.
{\*\atnid OK}{\*\atnauthor Olena Koval}\chatn{\*\annotation{\*\atndate 1234}\pard Ask about the deposit}\par}"#;

#[test]
fn a_word_document_says_who_made_it_where_and_with_what() {
    let doc = parse_rtf(WORD);
    let said: Vec<(&str, &str)> = doc
        .facts
        .iter()
        .map(|f| (f.kind, f.text.as_str()))
        .collect();
    for want in [
        ("title", "Lease чернетка"),
        ("author", "Олена Koval"),
        ("editor", "Petro Shevchuk"),
        ("company", "Northwind"),
        ("created", "2026-09-14 09:12"),
        ("modified", "2026-10-02 18:47"),
        ("printed", "2026-10-01 16:03"),
        ("revisions", "14"),
        ("editing", "3 h 12 min"),
        ("application", "Microsoft Word 16.0"),
        ("custom", "Client: Northwind"),
        ("template", "C:\\Users\\olena\\Templates\\Lease.dotx"),
        (
            "tracked",
            "1 insertion and 1 deletion by Olena Koval, not accepted yet",
        ),
        ("comments", "1 comment by Olena Koval"),
        ("deleted", "“18 000 UAH; offer that first”"),
    ] {
        assert!(said.contains(&want), "{want:?} not in {said:?}");
    }
    assert!(doc.tree.nodes().iter().all(|n| n.kind != NodeKind::Error));
}

#[test]
fn braces_that_do_not_balance_are_damage() {
    let doc = parse_rtf(br"{\rtf1\ansi{\info{\title Cut");
    assert!(doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
}

#[test]
fn text_decodes_escapes_and_leaves_out_nested_destinations() {
    assert_eq!(
        text(br"caf\'e9 \{x\} {\*\bkmkstart a}end", 1252),
        "café {x} end"
    );
    assert_eq!(text(br"\u1050\'3f\u1080\'3f\u1111?\u1074?", 1251), "Київ");
}

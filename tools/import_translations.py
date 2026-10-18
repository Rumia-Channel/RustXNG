#!/usr/bin/env python3
"""Generate src/i18n.rs from the SearXNG gettext catalogs.

The Rust port does not use GNU gettext at runtime; instead this script bakes a
small subset of the upstream catalogs
(searx/translations/*/LC_MESSAGES/messages.po) that corresponds to the English
UI strings used by the templates in this repository into static Rust tables.

Usage:
    python3 tools/import_translations.py /path/to/searxng [templates_dir] [output.rs]

The script
  1. extracts every translatable string from <templates_dir>/*.html (text nodes
     outside script/style elements plus the placeholder, title, aria-label, alt
     and value attributes),
  2. adds a small list of UI strings that are produced by the Rust code instead
     of the templates (see EXTRA_MSGIDS),
  3. looks up each string in the catalog of every shipped locale and
  4. writes src/i18n.rs.

Upstream catalogs contain no msgid_plural entries at all, so no plural
translations can be imported; the generated ngettext therefore selects the
singleton translation for the singular form and falls back to the English plural
for the remaining forms.  The plural-form selection rules (the Plural-Forms
headers) are imported verbatim.
"""
import html
import re
import sys
from pathlib import Path

# Locales shipped with the port.  "en" is the source language (empty catalog)
# and keeps available_locales() in sync with upstream's translation dirs.
LOCALES = [
    "de",
    "en",
    "es",
    "fr",
    "it",
    "ja",
    "ko",
    "pt_BR",
    "ru",
    "uk",
    "zh_Hans_CN",
]

# UI strings that the Rust code renders (they do not appear in the templates):
# src/web/mod.rs (time range options) and src/web/handlers.rs (safesearch /
# method options, stats page).
EXTRA_MSGIDS = [
    "Any time",
    "Past day",
    "Past week",
    "Past month",
    "Past year",
    "Off",
    "Moderate",
    "Strict",
    "POST",
    "GET",
    "Engine requests",
    "Engine errors",
    "Engine timeouts",
    "Results served",
    "Cache hits",
    "Cache entries",
    "Searches",
    "Uptime",
    "No engine has been used yet.",
    "Back to the search page",
]

# UI strings of the bundled plugins and answerers (the Lead renders the plugin
# name / description on the preferences page).
EXTRA_MSGIDS += [
    "Unit converter plugin",
    "Convert between units",
    "Ahmia blacklist",
    "Filter out onion results that appear in Ahmia's blacklist.",
    "Tor check plugin",
    "This plugin checks if the address of the request is a Tor exit-node, and"
    " informs the user if it is; like check.torproject.org, but from SearXNG.",
    "Infinite scroll",
    "Automatically loads the next page when scrolling to bottom of the current page",
    "Could not download the list of Tor exit-nodes from",
    "You are using Tor and it looks like you have the external IP address",
    "You are not using Tor and you have the external IP address",
    "Generate different random values",
    "Compute {func} of the arguments",
]

# Our templates are a rewrite, so a few UI strings differ from the upstream
# wording while meaning the same thing.  Map them onto the upstream msgid so the
# translation can be reused (the mapping is by hand, one entry per string).
ALIASES = {
    "Safe search": "SafeSearch",
    "Interface locale": "Interface language",
    "Search method": "HTTP Method",
    "Proxy image results": "Proxy image results through SearXNG",
    "Reset to defaults": "Reset defaults",
    "Any time": "Anytime",
    "Past day": "Last day",
    "Past week": "Last week",
    "Past month": "Last month",
    "Past year": "Last year",
    "Off": "None",
    "Previous": "Previous page",
    "Next": "Next page",
}

# (singular, plural) pairs that ngettext is used with.  Only the singular form
# exists in the upstream catalogs, see the module docstring.
PLURAL_PAIRS = [
    ("Engine", "Engines"),
    ("Time", "Times"),
]

# plural_index bodies, one per locale, copied from the upstream Plural-Forms
# header (the generated function repeats that comment).
PLURAL_RULES = {
    "de": "if n == 1 { 0 } else { 1 }",
    "en": "if n == 1 { 0 } else { 1 }",
    "es": "if n == 1 { 0 } else { 1 }",
    "it": "if n == 1 { 0 } else { 1 }",
    "ja": "0",
    "ko": "0",
    "zh_Hans_CN": "0",
    "fr": "if n > 1 { 1 } else { 0 }",
    "pt_BR": "if n > 1 { 1 } else { 0 }",
    "ru": (
        "if n % 10 == 1 && n % 100 != 11 { 0 }"
        " else if n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) { 1 }"
        " else if n % 10 == 0 || (n % 10 >= 5 && n % 10 <= 9) || (n % 100 >= 11 && n % 100 <= 14) { 2 }"
        " else { 3 }"
    ),
    "uk": (
        "if n % 10 == 1 && n % 100 != 11 { 0 }"
        " else if n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14) { 1 }"
        " else if n % 10 == 0 || (n % 10 >= 5 && n % 10 <= 9) || (n % 100 >= 11 && n % 100 <= 14) { 2 }"
        " else { 3 }"
    ),
}


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def unquote(value: str) -> str:
    """Decode a (possibly multi-line) PO string literal fragment."""
    value = value.strip()
    if len(value) >= 2 and value.startswith('"') and value.endswith('"'):
        value = value[1:-1]
    out = []
    index = 0
    escapes = {"n": "\n", "t": "\t", "r": "\r", '"': '"', "\\": "\\", "a": "\a", "b": "\b", "f": "\f", "v": "\v"}
    while index < len(value):
        char = value[index]
        if char == "\\" and index + 1 < len(value):
            out.append(escapes.get(value[index + 1], value[index + 1]))
            index += 2
        else:
            out.append(char)
            index += 1
    return "".join(out)


def parse_po(path: Path) -> dict:
    """Return {msgid: msgstr} for the non-fuzzy translations of one catalog."""
    entries = {}
    for block in re.split(r"\n\s*\n", read_text(path)):
        if "msgid" not in block:
            continue
        if any(line.startswith("#,") and "fuzzy" in line for line in block.splitlines()):
            continue
        msgid = None
        msgstr = None
        current = None
        for line in block.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("msgid_plural"):
                current = "plural"
                continue
            if line.startswith("msgid "):
                current = "msgid"
                msgid = unquote(line[len("msgid "):])
                continue
            if line.startswith("msgstr["):
                current = "plural_str"
                continue
            if line.startswith("msgstr "):
                current = "msgstr"
                msgstr = unquote(line[len("msgstr "):])
                continue
            if line.startswith('"'):
                if current == "msgid":
                    msgid += unquote(line)
                elif current == "msgstr":
                    msgstr += unquote(line)
                continue
        if msgid and msgstr:
            entries[msgid] = msgstr
    return entries


def plural_forms(path: Path) -> str | None:
    """The Plural-Forms header value of a catalog, or None."""
    header = ""
    for line in read_text(path).splitlines():
        line = line.strip()
        if line.startswith('"') and line.endswith('"'):
            header += unquote(line)
    match = re.search(r"Plural-Forms:\s*(.*?)\\n", header)
    return match.group(1).strip() if match else None


def template_strings(directory: Path) -> list:
    """Translatable strings of the askama templates, in template order."""
    seen = {}
    for path in sorted(directory.glob("*.html")):
        text = read_text(path)
        text = re.sub(r"\{#.*?#\}", "", text, flags=re.S)
        text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
        text = re.sub(r"<(script|style)\b.*?</\1>", "", text, flags=re.S | re.I)
        text = re.sub(r"\{\{.*?\}\}", " ", text, flags=re.S)
        text = re.sub(r"\{%.*?%\}", " ", text, flags=re.S)
        for attr in re.finditer(r'(?:placeholder|title|aria-label|alt)="([^"]+)"', text):
            add(seen, html.unescape(attr.group(1)))
        for chunk in re.split(r"<[^>]*>", text):
            add(seen, html.unescape(chunk))
    return list(seen)


# Characters that decorate a string but are not part of the message
# (em dashes, arrows, quote marks, ...).  Full stops / colons / question marks
# are kept because they are part of the message.
DECORATION = set("\u2014\u2013-\u00b7\u2022\u2190\u2192\u00ab\u00bb\"'\u201c\u201d\u2018\u2019\u00a0|/\\,;")


def clean(raw: str) -> str:
    """Collapse whitespace and drop decorative punctuation around the string."""
    value = " ".join(raw.split())
    value = re.sub(r"\s*\(\s*\)\s*$", "", value)
    while value and (value[0] in DECORATION or value[0] in "()"):
        value = value[1:].lstrip()
    while value and value[-1] in DECORATION:
        value = value[:-1].rstrip()
    return value


def add(seen: dict, raw: str) -> None:
    value = clean(raw)
    if not value:
        return
    if not any(char.isalpha() for char in value):
        return
    if value.startswith("{") or value.startswith("%"):
        return
    seen[value] = True


def lookup(catalog: dict, msgid: str) -> str | None:
    """Exact match, then a unique case-insensitive match, then the msgid without
    its final punctuation (upstream templates often add a full stop or colon)."""
    if msgid in catalog:
        return catalog[msgid]
    aliased = ALIASES.get(msgid)
    if aliased is not None and aliased in catalog:
        return catalog[aliased]
    lowered = msgid.lower()
    matches = [value for key, value in catalog.items() if key.lower() == lowered]
    if len(set(matches)) == 1 and matches:
        return matches[0]
    stripped = msgid.rstrip(".:?!... ")
    if stripped != msgid:
        matches = [value for key, value in catalog.items() if key.rstrip(".:?!... ") == stripped]
        if len(set(matches)) == 1 and matches:
            return matches[0]
    return None


def rust_string(value: str) -> str:
    out = ['"']
    for char in value:
        if char == "\\":
            out.append("\\\\")
        elif char == '"':
            out.append('\\"')
        elif char == "\n":
            out.append("\\n")
        elif char == "\r":
            out.append("\\r")
        elif char == "\t":
            out.append("\\t")
        elif ord(char) < 0x20 or ord(char) == 0x7F:
            out.append("\\u{%x}" % ord(char))
        else:
            out.append(char)
    out.append('"')
    return "".join(out)


def const_name(locale: str) -> str:
    return "MESSAGES_%s" % re.sub(r"\W", "_", locale).upper()


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    upstream = Path(sys.argv[1])
    templates = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("templates")
    output = Path(sys.argv[3]) if len(sys.argv) > 3 else Path("src/i18n.rs")

    msgids = template_strings(templates)
    for extra in EXTRA_MSGIDS:
        if extra not in msgids:
            msgids.append(extra)

    catalogs = {}
    for locale in LOCALES:
        po = upstream / "searx" / "translations" / locale / "LC_MESSAGES" / "messages.po"
        if not po.exists():
            print("warning: no catalog for %s" % locale, file=sys.stderr)
            continue
        catalogs[locale] = parse_po(po)

    shipped = [locale for locale in LOCALES if locale in catalogs]
    lines = []
    lines.append("//! gettext-style message catalog, ported from searx/translations/*/LC_MESSAGES/messages.po.")
    lines.append("//!")
    lines.append("//! Generated by tools/import_translations.py; do not edit by hand.")
    lines.append("//!")
    lines.append("//! Only strings used by templates/*.html (plus a handful of UI strings the")
    lines.append("//! Rust code renders, see the generator) are kept.  A locale without a")
    lines.append("//! translation for a msgid falls back to the English msgid, and a locale the")
    lines.append("//! instance does not ship falls back to 'en'.")
    lines.append("")
    lines.append("/// The translator of one UI locale.")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    lines.append("pub struct Translator {")
    lines.append("    locale: &'static str,")
    lines.append("}")
    lines.append("")
    lines.append("/// The locales shipped with the port (the source locale 'en' included).")
    lines.append("static SHIPPED_LOCALES: &[&str] = &[")
    lines.append("    " + ", ".join('"%s"' % locale for locale in shipped) + ",")
    lines.append("];")
    lines.append("")

    stats = {}
    for locale in shipped:
        catalog = catalogs[locale]
        pairs = []
        for msgid in msgids:
            value = lookup(catalog, msgid)
            if value is not None and value != msgid:
                pairs.append((msgid, value))
        stats[locale] = len(pairs)
        lines.append("/// Translations of %s." % locale)
        lines.append("static %s: &[(&str, &str)] = &[" % const_name(locale))
        for msgid, value in pairs:
            lines.append("    (%s, %s)," % (rust_string(msgid), rust_string(value)))
        lines.append("];")
        lines.append("")

    lines.append("struct Catalog {")
    lines.append("    locale: &'static str,")
    lines.append("    messages: &'static [(&'static str, &'static str)],")
    lines.append("}")
    lines.append("")
    lines.append("static CATALOGS: &[Catalog] = &[")
    for locale in shipped:
        lines.append("    Catalog {")
        lines.append('        locale: "%s",' % locale)
        lines.append("        messages: %s," % const_name(locale))
        lines.append("    },")
    lines.append("];")
    lines.append("")
    lines.append("/// Plural form selection, copied from the Plural-Forms header of each")
    lines.append("/// upstream catalog.")
    lines.append("fn plural_index(locale: &str, n: u64) -> usize {")
    lines.append("    match locale {")
    for locale in shipped:
        rule = PLURAL_RULES.get(locale)
        if rule is None:
            continue
        lines.append('        "%s" => %s,' % (locale, rule))
    lines.append("        _ => if n == 1 { 0 } else { 1 },")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// The (singular, plural) pairs the UI uses with ngettext.")
    lines.append("pub static NGETTEXT_PAIRS: &[(&str, &str)] = &[")
    for singular, plural in PLURAL_PAIRS:
        lines.append("    (%s, %s)," % (rust_string(singular), rust_string(plural)))
    lines.append("];")
    lines.append("")
    lines.append("/// Plural form translations.  The upstream catalogs contain no msgid_plural")
    lines.append("/// entries, so this table stays empty and ngettext falls back to the English")
    lines.append("/// plural for every form except the singular (which uses the catalog entry).")
    lines.append("fn plural_forms(_locale: &str, _msgid: &str) -> Option<&'static [&'static str]> {")
    lines.append("    None")
    lines.append("}")
    lines.append("")
    lines.append("/// The catalog of a locale, or 'en' for an unknown one.")
    lines.append("pub fn for_locale(locale: &str) -> Translator {")
    lines.append("    Translator {")
    lines.append("        locale: match_locale(locale),")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// Resolve a (possibly regional) locale tag onto a shipped locale.")
    lines.append("fn match_locale(locale: &str) -> &'static str {")
    lines.append("    let normalized = locale.trim().replace('-', \"_\");")
    lines.append("    if normalized.is_empty() {")
    lines.append("        return \"en\";")
    lines.append("    }")
    lines.append("    for candidate in SHIPPED_LOCALES {")
    lines.append("        if candidate.eq_ignore_ascii_case(&normalized) {")
    lines.append("            return candidate;")
    lines.append("        }")
    lines.append("    }")
    lines.append("    let language = normalized.split('_').next().unwrap_or(\"\");")
    lines.append("    if language.eq_ignore_ascii_case(\"zh\") {")
    lines.append("        return \"zh_Hans_CN\";")
    lines.append("    }")
    lines.append("    for candidate in SHIPPED_LOCALES {")
    lines.append("        if candidate.eq_ignore_ascii_case(language) {")
    lines.append("            return candidate;")
    lines.append("        }")
    lines.append("    }")
    lines.append("    \"en\"")
    lines.append("}")
    lines.append("")
    lines.append("/// The locales the port ships translations for.")
    lines.append("pub fn available_locales() -> &'static [&'static str] {")
    lines.append("    SHIPPED_LOCALES")
    lines.append("}")
    lines.append("")
    lines.append("impl Translator {")
    lines.append("    /// The resolved locale of this translator.")
    lines.append("    pub fn locale(&self) -> &'static str {")
    lines.append("        self.locale")
    lines.append("    }")
    lines.append("")
    lines.append("    fn catalog(&self) -> Option<&'static Catalog> {")
    lines.append("        CATALOGS.iter().find(|catalog| catalog.locale == self.locale)")
    lines.append("    }")
    lines.append("")
    lines.append("    /// The translation of msgid, or msgid itself when the catalog has none.")
    lines.append("    pub fn gettext<'a>(&'a self, msgid: &'a str) -> &'a str {")
    lines.append("        match self.catalog() {")
    lines.append("            Some(catalog) => catalog")
    lines.append("                .messages")
    lines.append("                .iter()")
    lines.append("                .find(|(key, _)| *key == msgid)")
    lines.append("                .map(|(_, value)| *value)")
    lines.append("                .unwrap_or(msgid),")
    lines.append("            None => msgid,")
    lines.append("        }")
    lines.append("    }")
    lines.append("")
    lines.append("    /// The singular or plural translation of singular / plural for n.")
    lines.append("    pub fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> &'a str {")
    lines.append("        let index = plural_index(self.locale, n);")
    lines.append("        if index == 0 {")
    lines.append("            return self.gettext(singular);")
    lines.append("        }")
    lines.append("        if let Some(forms) = plural_forms(self.locale, singular) {")
    lines.append("            if let Some(form) = forms.get(index) {")
    lines.append("                if !form.is_empty() {")
    lines.append("                    return form;")
    lines.append("                }")
    lines.append("            }")
    lines.append("        }")
    lines.append("        plural")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("#[cfg(test)]")
    lines.append("mod tests {")
    lines.append("    use super::*;")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn english_is_the_fallback_for_unknown_locales() {")
    lines.append("        let translator = for_locale(\"xx\");")
    lines.append("        assert_eq!(translator.locale(), \"en\");")
    lines.append("        assert_eq!(translator.gettext(\"Preferences\"), \"Preferences\");")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn regional_tags_resolve_onto_a_shipped_locale() {")
    lines.append("        assert_eq!(for_locale(\"de-DE\").locale(), \"de\");")
    lines.append("        assert_eq!(for_locale(\"pt-BR\").locale(), \"pt_BR\");")
    lines.append("        assert_eq!(for_locale(\"zh-CN\").locale(), \"zh_Hans_CN\");")
    lines.append("        assert_eq!(for_locale(\"ja_JP\").locale(), \"ja\");")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn available_locales_contain_the_source_locale() {")
    lines.append("        let locales = available_locales();")
    lines.append("        assert!(locales.contains(&\"en\"));")
    lines.append("        assert!(locales.contains(&\"ja\"));")
    lines.append("        assert!(locales.contains(&\"de\"));")
    lines.append("        assert!(locales.contains(&\"zh_Hans_CN\"));")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn ngettext_uses_the_english_plural_for_the_plural_form() {")
    lines.append("        let translator = for_locale(\"en\");")
    lines.append("        assert_eq!(translator.ngettext(\"Engine\", \"Engines\", 1), \"Engine\");")
    lines.append("        assert_eq!(translator.ngettext(\"Engine\", \"Engines\", 2), \"Engines\");")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn ngettext_follows_the_locale_plural_rule() {")
    lines.append("        // Japanese has nplurals=1: every count uses the singular form.")
    lines.append("        let japanese = for_locale(\"ja\");")
    lines.append("        let singular = japanese.gettext(\"Engine\");")
    lines.append("        assert_eq!(japanese.ngettext(\"Engine\", \"Engines\", 1), singular);")
    lines.append("        assert_eq!(japanese.ngettext(\"Engine\", \"Engines\", 5), singular);")
    lines.append("        // French uses the plural for n > 1 (nplurals=2).")
    lines.append("        let french = for_locale(\"fr\");")
    lines.append("        assert_eq!(french.ngettext(\"Engine\", \"Engines\", 1), french.gettext(\"Engine\"));")
    lines.append("        assert_eq!(french.ngettext(\"Engine\", \"Engines\", 2), \"Engines\");")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn every_ngettext_pair_is_translatable() {")
    lines.append("        let translator = for_locale(\"ja\");")
    lines.append("        for (singular, plural) in NGETTEXT_PAIRS {")
    lines.append("            let one = translator.ngettext(singular, plural, 1);")
    lines.append("            let many = translator.ngettext(singular, plural, 7);")
    lines.append("            assert!(!one.is_empty() && !many.is_empty());")
    lines.append("            // nplurals=1: both forms come from the catalog entry.")
    lines.append("            assert_eq!(one, many);")
    lines.append("        }")
    lines.append("    }")
    lines.append("")
    lines.append("    #[test]")
    lines.append("    fn translations_come_from_the_upstream_catalogs() {")
    lines.append("        assert_eq!(for_locale(\"de\").gettext(\"Preferences\"), \"Einstellungen\");")
    lines.append("        assert_eq!(for_locale(\"de\").gettext(\"About\"), \"Über\");")
    lines.append("        assert_ne!(for_locale(\"ja\").gettext(\"Preferences\"), \"Preferences\");")
    lines.append("        // An unknown msgid is returned unchanged.")
    lines.append("        assert_eq!(for_locale(\"de\").gettext(\"no such msgid\"), \"no such msgid\");")
    lines.append("    }")
    lines.append("}")
    lines.append("")

    output.write_text("\n".join(lines), encoding="utf-8")
    print("wrote %s: %d msgids, %d locales" % (output, len(msgids), len(shipped)))
    for locale in shipped:
        print("  %-12s %4d translations" % (locale, stats[locale]))
    return 0


if __name__ == "__main__":
    sys.exit(main())

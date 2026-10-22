# Bundled fonts

Open Sans Regular and Italic, licensed under the SIL Open Font License 1.1
(`OFL.txt`). The helper loads them before the system fonts because
mermaid-cli 12.0.0 loads Open Sans from `@fontsource/open-sans` into its
page, and the sequence, journey, timeline, and C4 diagrams default to it.

Source: <https://github.com/googlefonts/opensans>, `fonts/ttf/` at commit
`bd7e37632246368c60fdcbd374dbf9bad11969b6` (2023-11-16T14:54:02Z). `@fontsource/open-sans`
is built from the same upstream through the Google Fonts API.

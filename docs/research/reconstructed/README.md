# Reconstructed research documents

Lossless reconstructions of the four external research documents. Every
formula and numeric table cell in the sources is an embedded image; the plain
`.txt` exports one directory up drop all of them silently. Here each image is
transcribed and substituted back at its original position, written `{like
this}`, so the documents read as the continuous prose they are.

| file | source images | transcribed |
| --- | --- | --- |
| `2026-08-27_epistemic_governance.merged.txt` | 70 | 70 |
| `2026-08-27_nondirectional_execution.merged.txt` | 194 | 194 |
| `2026-08-27_statistical_foundations.merged.txt` | 116 | 116 |
| `2026-08-29_empirical_validation_taxonomy.merged.txt` | 227 | 227 |

Committing these is round three's own instruction: *"a rigorous adoption
protocol requires automated pre-ingestion visual asset audits, numeric
continuity checks across table ranges, and committing reconstructed, lossless
research documents directly into the repository's immutable research tree."*

**These are our transcription, not the source.** The `.txt` exports beside them
are kept verbatim precisely so the lossy original and our reconstruction can be
told apart. Where a number here matters, check it against
[`../EXTRACTION_LOSS.md`](../EXTRACTION_LOSS.md), which records how each was
recovered.

Method, reproducible from any Google Doc:

```sh
curl -sL -o doc.docx "https://docs.google.com/document/d/<ID>/export?format=docx"
unzip -o doc.docx -d doc/
# map a:blip r:embed -> word/_rels/document.xml.rels -> word/media/imageN.png
# images are black-on-transparent RGBA: composite onto white before viewing
```

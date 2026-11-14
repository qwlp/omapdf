#!/usr/bin/env python3
"""Create deterministic, original CC0 reading fixtures with Python's standard library.

SPDX-License-Identifier: CC0-1.0
The generated text, graphics, PDF and EPUB files are dedicated to the public domain.
"""
from pathlib import Path
import html
import zipfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
EXAMPLES = ROOT / 'examples'


def pdf_string(text):
    return '(' + text.replace('\\', '\\\\').replace('(', '\\(').replace(')', '\\)') + ')'


def write_pdf(path, pages, image_only=False):
    objects = []

    def add(data=b''):
        objects.append(data.encode() if isinstance(data, str) else data)
        return len(objects)

    def stream(data, extra=''):
        packed = zlib.compress(data)
        return add(f'<< /Length {len(packed)} /Filter /FlateDecode {extra} >>\nstream\n'.encode() + packed + b'\nendstream')

    catalog, page_tree = add(), add()
    font = add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
    image = None
    if image_only:
        # Original geometric color chart: no photographed or sourced material.
        width, height = 192, 128
        pixels = bytes(value for y in range(height) for x in range(width)
                       for value in (40 + x * 200 // width, 40 + y * 180 // height, 70 + ((x // 24 + y // 16) % 2) * 130))
        image = stream(pixels, f'/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8')
    page_ids = []
    for number in range(pages):
        page_id = add()
        page_ids.append(page_id)
        if image_only:
            commands = b'q 480 0 0 320 60 180 cm /Chart Do Q'
            resources = f'<< /XObject << /Chart {image} 0 R >> >>'
        else:
            lines = [
                'Omapdf Reading Lab',
                f'Page {number + 1} of {pages}',
                'Original test material. CC0 public-domain dedication.',
                '',
                'A quiet desk leaves room for a page, a pencil, and a question.',
                'Read one line, pause, and follow the next idea at your own pace.',
                'Bookmarks keep a place. Highlights keep a thought worth revisiting.',
                'A preview lets two distant passages share the same small screen.',
                '',
                f'This is reading station {number + 1:04d}. Its marker is station{number + 1:04d}.',
                'The word permittivity is a search target in this generated document.',
                'Search, select, copy, and annotate these original sentences freely.',
            ]
            lines += [f'Exercise {line + 1:02d}: compare a sentence, keep a note, then continue reading.' for line in range(18)]
            commands = ('BT /F1 12 Tf 48 744 Td 18 TL\n' + '\n'.join(pdf_string(line) + ' Tj T*' for line in lines) + '\nET').encode()
            # A simple original diagram also exercises vector rendering.
            commands += b'\nq 0.15 0.45 0.65 rg 48 95 150 24 re f 0.55 0.7 0.3 rg 220 95 100 24 re f Q'
            resources = f'<< /Font << /F1 {font} 0 R >> >>'
        content = stream(commands)
        objects[page_id - 1] = f'<< /Type /Page /Parent {page_tree} 0 R /MediaBox [0 0 612 792] /Resources {resources} /Contents {content} 0 R >>'.encode()
    outlines = add()
    preface, chapter, section, subheading = add(), add(), add(), add()
    target = page_ids[min(29, pages - 1)]
    objects[preface - 1] = f'<< /Title (Preface) /Parent {outlines} 0 R /Next {chapter} 0 R /Dest [{page_ids[0]} 0 R /Fit] >>'.encode()
    objects[chapter - 1] = f'<< /Title (1. READING LAB) /Parent {outlines} 0 R /Prev {preface} 0 R /First {section} 0 R /Last {section} 0 R /Count 2 /Dest [{target} 0 R /Fit] >>'.encode()
    objects[section - 1] = f'<< /Title (1.1 History of a reading session) /Parent {chapter} 0 R /First {subheading} 0 R /Last {subheading} 0 R /Count 1 /Dest [{target} 0 R /Fit] >>'.encode()
    objects[subheading - 1] = f'<< /Title (Starting points) /Parent {section} 0 R /Dest [{target} 0 R /Fit] >>'.encode()
    objects[outlines - 1] = f'<< /Type /Outlines /First {preface} 0 R /Last {chapter} 0 R /Count 4 >>'.encode()
    objects[catalog - 1] = f'<< /Type /Catalog /Pages {page_tree} 0 R /Outlines {outlines} 0 R >>'.encode()
    objects[page_tree - 1] = f'<< /Type /Pages /Count {pages} /Kids [{" ".join(f"{page} 0 R" for page in page_ids)}] >>'.encode()
    info = add('<< /Title (Omapdf Reading Lab) /Author (Omapdf contributors) /Subject (Original CC0 test fixture) >>')
    output = bytearray(b'%PDF-1.4\n%\xe2\xe3\xcf\xd3\n')
    offsets = [0]
    for index, data in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f'{index} 0 obj\n'.encode() + data + b'\nendobj\n')
    xref = len(output)
    output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:
        output.extend(f'{offset:010d} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(offsets)} /Root {catalog} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    path.write_bytes(output)


def write_epub(path):
    titles = ['A place to read', 'Light on the desk', 'Margins', 'Small questions',
              'Following a trail', 'Keeping a note', 'Turning a page', 'Compilation',
              'Waypoints', 'Two views', 'A line at a time', 'A useful pause',
              'Finding a passage', 'Color and contrast', 'A quiet morning', 'Loose leaves',
              'Reading together', 'A second look', 'Keeping your place', 'Distant pages',
              'The shape of a book', 'A small diagram', 'Links and paths', 'A longer walk',
              'Checking a detail', 'An open window', 'Returning home', 'After the last page']
    files = {
        'mimetype': 'application/epub+zip',
        'META-INF/container.xml': '<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>',
        'OEBPS/style.css': 'body { font-family: serif; line-height: 1.45; } h1 { color: #345e78; } p { margin: 0 0 0.7em; } pre { font-family: monospace; }',
    }
    for index, title in enumerate(titles):
        paragraphs = []
        for number in range(40):
            paragraphs.append(f'<p id="note-{number}">Reading station {index + 1}, note {number + 1}. '
                              'A page gives an idea a place to rest. We can look closely at one sentence, '
                              'leave a short mark beside it, and return after exploring another passage. '
                              'The view may change its size, but the words still offer a familiar path. '
                              'A useful reader keeps that path easy to find, even when the next question leads elsewhere.</p>')
        content = (f'<h1 id="start">{html.escape(title)}</h1>'
                   '<p>Original writing for the Omapdf reading laboratory. Dedicated to the public domain under CC0.</p>'
                   f'<p><a href="chapter{(index + 1) % len(titles):02d}.xhtml#start">Continue to another station</a> '
                   '<a href="https://example.org/">Example external link</a></p>'
                   + ''.join(paragraphs) + '<pre>let page = 1;\nlet next = page + 1;</pre>')
        files[f'OEBPS/chapter{index:02d}.xhtml'] = f'<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>{html.escape(title)}</title><link rel="stylesheet" href="style.css"/></head><body>{content}</body></html>'
    nav = ''.join(f'<li><a href="chapter{index:02d}.xhtml#start">{html.escape(title)}</a></li>' for index, title in enumerate(titles))
    files['OEBPS/nav.xhtml'] = f'<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc"><ol>{nav}</ol></nav></body></html>'
    manifest = ''.join(f'<item id="c{index}" href="chapter{index:02d}.xhtml" media-type="application/xhtml+xml"/>' for index in range(len(titles)))
    spine = ''.join(f'<itemref idref="c{index}"/>' for index in range(len(titles)))
    files['OEBPS/content.opf'] = f'''<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:omapdf:reading-lab:1</dc:identifier><dc:title>Omapdf Reading Lab</dc:title><dc:creator>Omapdf contributors</dc:creator><dc:language>en</dc:language><dc:rights>CC0-1.0 public-domain dedication</dc:rights><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata><manifest>{manifest}<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="style" href="style.css" media-type="text/css"/></manifest><spine>{spine}</spine></package>'''
    with zipfile.ZipFile(path, 'w') as archive:
        for name, text in files.items():
            entry = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_STORED if name == 'mimetype' else zipfile.ZIP_DEFLATED
            entry.external_attr = 0o644 << 16
            archive.writestr(entry, text.encode())


def main():
    EXAMPLES.mkdir(exist_ok=True)
    write_pdf(EXAMPLES / 'reading-lab.pdf', 32)
    write_pdf(EXAMPLES / 'large-reading-lab.pdf', 1220)
    write_pdf(EXAMPLES / 'color-chart.pdf', 4, image_only=True)
    write_epub(EXAMPLES / 'reading-lab.epub')
    for name in ['reading-lab.pdf', 'large-reading-lab.pdf', 'color-chart.pdf', 'reading-lab.epub']:
        print(f'{name}: {(EXAMPLES / name).stat().st_size:,} bytes')


if __name__ == '__main__':
    main()

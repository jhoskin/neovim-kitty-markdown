local parser = require('kitty-markdown.parser')

local here = debug.getinfo(1, 'S').source:sub(2)
local fixtures = vim.fs.joinpath(vim.fs.dirname(vim.fn.fnamemodify(here, ':p')), 'fixtures')

local function load_fixture(name)
  local buf = vim.fn.bufadd(vim.fs.joinpath(fixtures, name))
  vim.fn.bufload(buf)
  vim.bo[buf].filetype = 'markdown'
  return buf
end

local function parse_lines(lines)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = 'markdown'
  return parser.parse(buf)
end

local function kinds(blocks)
  local out = {}
  for i, block in ipairs(blocks) do
    out[i] = block.kind
  end
  return out
end

local function types(inlines)
  local out = {}
  for i, item in ipairs(inlines) do
    out[i] = item.type
  end
  return out
end

local function text_of(inlines)
  local parts = {}
  for _, item in ipairs(inlines) do
    if item.type == 'text' or item.type == 'code' then
      parts[#parts + 1] = item.text
    elseif item.children then
      parts[#parts + 1] = text_of(item.children)
    end
  end
  return table.concat(parts)
end

describe('parser', function()
  describe('headings', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('headings.md'))
    end)

    it('reads ATX and setext levels', function()
      local levels = {}
      for _, block in ipairs(blocks) do
        if block.kind == 'heading' then
          levels[#levels + 1] = block.level
        end
      end
      assert.are.same({ 1, 2, 3, 3, 1, 2, 4, 5, 6 }, levels)
    end)

    it('keeps inline markup inside the heading', function()
      assert.are.same({ 'text', 'emph' }, types(blocks[3].inlines))
      assert.are.equal('two', text_of(blocks[3].inlines[2].children))
    end)

    it('drops the closing hashes of an ATX heading', function()
      local last = blocks[#blocks]
      assert.are.equal(6, last.level)
      assert.are.equal('Six', text_of(last.inlines))
    end)

    it('gives duplicate headings numbered slugs', function()
      assert.are.equal('dup', blocks[4].slug)
      assert.are.equal('dup-1', blocks[5].slug)
      assert.are.equal('setext-one', blocks[6].slug)
    end)

    it('records 1-based source rows per block', function()
      assert.are.equal(1, blocks[1].src_start)
      assert.are.equal(1, blocks[1].src_end)
      assert.are.equal(3, blocks[2].src_start)
      -- the setext heading spans its text and its underline
      assert.are.equal(11, blocks[6].src_start)
      assert.are.equal(12, blocks[6].src_end)
    end)
  end)

  describe('inline', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('inline.md'))
    end)

    it('marks emphasis, strong, strike, and code spans', function()
      local items = blocks[1].inlines
      assert.are.same(
        { 'text', 'emph', 'text', 'strong', 'text', 'strike', 'text', 'code', 'text' },
        types(items)
      )
      assert.are.equal('emph', text_of(items[2].children))
      assert.are.equal('strong', text_of(items[4].children))
      assert.are.equal('strike', text_of(items[6].children))
      assert.are.equal('code span', items[8].text)
    end)

    it('marks hard line breaks from spaces and from a backslash', function()
      local items = blocks[2].inlines
      local breaks = 0
      for _, item in ipairs(items) do
        if item.type == 'linebreak' then
          breaks = breaks + 1
        end
      end
      assert.are.equal(2, breaks)
      assert.are.equal('Hard break with spaces', items[1].text)
    end)

    it('marks emoji shortcodes', function()
      local items = blocks[3].inlines
      assert.are.equal('emoji', items[2].type)
      assert.are.equal('tada', items[2].shortcode)
      assert.are.equal('unknown_thing', items[4].shortcode)
    end)

    it('shows a backslash escape as the escaped character', function()
      assert.are.equal('Escapes * _ # and entities & < © A B &bogus;.', text_of(blocks[4].inlines))
    end)

    it('keeps inline HTML as html items', function()
      local items = blocks[5].inlines
      assert.are.equal('html', items[2].type)
      assert.are.equal('<b>', items[2].text)
      assert.are.equal('<br>', items[6].text)
    end)

    it('nests strong inside emphasis', function()
      local items = blocks[6].inlines
      assert.are.equal('emph', items[2].type)
      assert.are.equal('strong', items[2].children[1].type)
      assert.are.equal('both', text_of(items[2].children))
    end)
  end)

  describe('lists', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('lists.md'))
    end)

    it('nests lists as item children', function()
      local list = blocks[1]
      assert.are.equal('list', list.kind)
      assert.is_false(list.ordered)
      assert.are.equal(3, #list.items)
      local nested = list.items[2].children[1]
      assert.are.equal('list', nested.kind)
      assert.are.equal('two.a', text_of(nested.items[1].inlines))
      local deeper = nested.items[1].children[1].items[1].children[1]
      assert.are.equal('two.a.i.x', text_of(deeper.items[1].inlines))
    end)

    it('reads the ordered start number', function()
      assert.is_true(blocks[2].ordered)
      assert.are.equal(1, blocks[2].start)
      assert.is_true(blocks[3].ordered)
      assert.are.equal(7, blocks[3].start)
    end)

    it('reads task markers', function()
      local items = blocks[4].items
      assert.is_true(items[1].checked)
      assert.is_false(items[2].checked)
      assert.is_nil(blocks[1].items[1].checked)
    end)

    it('keeps an item with only a nested list', function()
      -- one blank line between items keeps them in the same (loose) list
      local item = blocks[4].items[4]
      assert.are.same({}, item.inlines)
      assert.are.equal('list', item.children[1].kind)
    end)

    it('keeps a second paragraph as an item child', function()
      local item = blocks[5].items[1]
      assert.are.equal('loose item', text_of(item.inlines))
      assert.are.equal('paragraph', item.children[1].kind)
      assert.are.equal('second paragraph', text_of(item.children[1].inlines))
    end)
  end)

  describe('tables', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('tables.md'))
    end)

    it('reads alignment from the delimiter row', function()
      assert.are.same({ 'left', 'center', 'right' }, blocks[1].align)
    end)

    it('reads header and rows as inline lists', function()
      local t = blocks[1]
      assert.are.equal(3, #t.header)
      assert.are.equal('Center', text_of(t.header[2]))
      assert.are.equal(3, #t.rows)
      assert.are.equal('strong', t.rows[2][2][1].type)
      assert.are.equal('日本語', text_of(t.rows[3][1]))
    end)

    it('keeps short and long rows as written', function()
      local t = blocks[2]
      assert.are.equal(1, #t.rows[1])
      assert.are.equal(3, #t.rows[2])
    end)
  end)

  describe('code', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('code.md'))
    end)

    it('reads the language and the lines of a fence', function()
      assert.are.equal('code', blocks[1].kind)
      assert.are.equal('lua', blocks[1].lang)
      assert.are.same({ 'local x = 1', 'print(x)' }, blocks[1].lines)
    end)

    it('leaves the language nil without an info string', function()
      assert.is_nil(blocks[2].lang)
      assert.are.same({ 'no language' }, blocks[2].lines)
    end)

    it('reads an indented code block without its indent', function()
      assert.are.equal('code', blocks[3].kind)
      assert.is_nil(blocks[3].lang)
      assert.are.same({ 'indented code', 'second line' }, blocks[3].lines)
    end)

    it('takes the first word of the info string as the language', function()
      assert.are.equal('python', blocks[4].lang)
    end)

    it('strips the container prefix from a fence inside a list item', function()
      local code = blocks[5].items[1].children[1]
      assert.are.equal('code', code.kind)
      assert.are.equal('sh', code.lang)
      assert.are.same({ 'echo nested' }, code.lines)
    end)
  end)

  describe('quotes and alerts', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('quotes_alerts.md'))
    end)

    it('joins quote lines into paragraphs and keeps nesting', function()
      local quote = blocks[1]
      assert.are.equal('blockquote', quote.kind)
      assert.are.same({ 'paragraph', 'paragraph' }, kinds(quote.children))
      assert.are.equal('A quote line continues here.', text_of(quote.children[1].inlines))
      assert.are.equal('blockquote', blocks[2].children[2].kind)
      assert.are.equal('Inner quote', text_of(blocks[2].children[2].children[1].inlines))
    end)

    it('turns a [!KIND] quote into an alert of that kind', function()
      local found = {}
      for _, block in ipairs(blocks) do
        if block.kind == 'alert' then
          found[#found + 1] = block.type
        end
      end
      assert.are.same({ 'NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION', 'NOTE' }, found)
      assert.are.equal('Note body.', text_of(blocks[3].children[1].inlines))
    end)

    it('allows an alert with no body', function()
      local last = blocks[#blocks]
      assert.are.equal('alert', last.kind)
      assert.are.same({}, last.children)
    end)

    it('keeps an unknown marker as a plain quote', function()
      local blocks2 = parse_lines({ '> [!NOPE]', '> text' })
      assert.are.equal('blockquote', blocks2[1].kind)
    end)
  end)

  describe('images', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('images.md'))
    end)

    it('reads standalone images with src and alt', function()
      assert.are.same({
        'paragraph',
        'image',
        'image',
        'image',
        'image',
        'paragraph',
        'image',
        'paragraph',
      }, kinds(blocks))
      assert.are.equal('img/local.png', blocks[2].src)
      assert.are.equal('Local alt', blocks[2].alt)
      assert.are.equal('/abs/pic.jpg', blocks[3].src)
      assert.are.equal('https://example.com/r.png', blocks[4].src)
    end)

    it('reads an img tag with width and height attrs', function()
      local tag = blocks[5]
      assert.are.equal('tagged.png', tag.src)
      assert.are.equal('Tagged', tag.alt)
      assert.are.same({ width = 120, height = 40 }, tag.attrs)
    end)

    it('lifts an inline image after its paragraph (R-095)', function()
      assert.are.equal('Inline  inside a paragraph.', text_of(blocks[6].inlines))
      assert.are.equal('image', blocks[7].kind)
      assert.are.equal('inline.png', blocks[7].src)
      assert.are.equal(11, blocks[7].src_start)
    end)

    it('lifts an inline img tag too', function()
      local blocks2 = parse_lines({ 'Text <img src="t.png" alt="T"> more.' })
      assert.are.same({ 'paragraph', 'image' }, kinds(blocks2))
      assert.are.same({ src = 't.png', alt = 'T', attrs = {} }, {
        src = blocks2[2].src,
        alt = blocks2[2].alt,
        attrs = blocks2[2].attrs,
      })
    end)

    it('keeps the alt text of an image inside a link', function()
      local blocks2 = parse_lines({ '[![pic](p.png)](http://z)' })
      assert.are.same({ 'paragraph', 'image' }, kinds(blocks2))
      local link = blocks2[1].inlines[1]
      assert.are.equal('link', link.type)
      assert.are.equal('pic', text_of(link.children))
    end)
  end)

  describe('mermaid', function()
    it('makes a mermaid block from a mermaid fence (R-097)', function()
      local blocks = parser.parse(load_fixture('mermaid.md'))
      assert.are.same({ 'paragraph', 'mermaid', 'mermaid' }, kinds(blocks))
      assert.are.equal('%% comment\nflowchart LR\n  A --> B', blocks[2].source)
      assert.are.equal('flowchart', blocks[2].alt)
      assert.are.equal('sequenceDiagram', blocks[3].alt)
      assert.are.equal(3, blocks[2].src_start)
      assert.are.equal(7, blocks[2].src_end)
    end)
  end)

  describe('footnotes', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('footnotes.md'))
    end)

    it('marks footnote references with their label', function()
      local refs = {}
      for _, item in ipairs(blocks[1].inlines) do
        if item.type == 'footnote_ref' then
          refs[#refs + 1] = item.label
        end
      end
      assert.are.same({ 'b', 'a', 'b' }, refs)
    end)

    it('splits consecutive definitions into footnote_def blocks', function()
      assert.are.same(
        { 'paragraph', 'footnote_def', 'footnote_def', 'footnote_def' },
        kinds(blocks)
      )
      assert.are.equal('a', blocks[2].label)
      assert.are.equal('Alpha note.', text_of(blocks[2].inlines))
      assert.are.equal('b', blocks[3].label)
      assert.are.equal('emph', blocks[3].inlines[2].type)
      assert.are.equal('unused', blocks[4].label)
    end)

    it('reads a one-word definition', function()
      local blocks2 = parse_lines({ 'x[^1]', '', '[^1]: word' })
      assert.are.equal('footnote_def', blocks2[2].kind)
      assert.are.equal('1', blocks2[2].label)
      assert.are.equal('word', text_of(blocks2[2].inlines))
    end)
  end)

  describe('front matter', function()
    it('makes a front_matter block and keeps the rest', function()
      local blocks = parser.parse(load_fixture('front_matter.md'))
      assert.are.same({ 'front_matter', 'heading', 'paragraph' }, kinds(blocks))
      assert.are.equal(1, blocks[1].src_start)
      assert.are.equal(4, blocks[1].src_end)
    end)
  end)

  describe('html', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('html.md'))
    end)

    it('keeps HTML blocks with their lines', function()
      assert.are.equal('html', blocks[1].kind)
      assert.are.same({ '<div class="x">', '  <p>block html</p>', '</div>' }, blocks[1].lines)
      -- the comment block at the end shows nothing (R-270)
      assert.are.equal(3, #blocks)
    end)

    it('turns an img HTML block into an image block', function()
      assert.are.equal('image', blocks[3].kind)
      assert.are.equal('block.png', blocks[3].src)
      assert.are.equal('Block img', blocks[3].alt)
    end)

    it('keeps inline tags inside the paragraph', function()
      assert.are.same({ 'text', 'html', 'text', 'html', 'text' }, types(blocks[2].inlines))
    end)
  end)

  describe('links', function()
    local blocks
    before_each(function()
      blocks = parser.parse(load_fixture('links.md'))
    end)

    it('reads inline links with target and text', function()
      local link = blocks[1].inlines[2]
      assert.are.equal('link', link.type)
      assert.are.equal('https://example.com/path', link.target)
      assert.are.equal('link text', text_of(link.children))
    end)

    it('reads autolinks, bare URLs, and email autolinks', function()
      local items = blocks[2].inlines
      local found = {}
      for _, item in ipairs(items) do
        if item.type == 'autolink' then
          found[#found + 1] = { item.text, item.url }
        end
      end
      assert.are.same({
        { 'https://auto.example', 'https://auto.example' },
        { 'https://bare.example/x', 'https://bare.example/x' },
        { 'me@example.com', 'mailto:me@example.com' },
      }, found)
    end)

    it('resolves full, collapsed, and shortcut reference links', function()
      local items = blocks[3].inlines
      local targets = {}
      for _, item in ipairs(items) do
        if item.type == 'link' then
          targets[#targets + 1] = item.target
        end
      end
      assert.are.same({
        'https://ref.example/r',
        'https://collapsed.example',
        'https://shortcut.example',
      }, targets)
    end)

    it('keeps an unresolved reference as text', function()
      assert.are.equal('Undefined [nothing][nope] stays.', text_of(blocks[5].inlines))
    end)

    it('does not make blocks out of link definitions', function()
      assert.are.same(
        { 'paragraph', 'paragraph', 'paragraph', 'paragraph', 'paragraph', 'heading' },
        kinds(blocks)
      )
    end)
  end)

  describe('math', function()
    it('reads inline and display math (R-062)', function()
      local blocks = parser.parse(load_fixture('math.md'))
      local inline = blocks[1].inlines[2]
      assert.are.equal('math', inline.type)
      assert.are.equal('E = mc^2', inline.tex)
      assert.is_false(inline.display)
      local display = blocks[2].inlines[1]
      assert.are.equal('math', display.type)
      assert.is_true(display.display)
      assert.are.equal('\\sum_{i=1}^{n} i', display.tex)
      assert.are.equal('\\alpha', blocks[3].inlines[2].tex)
    end)
  end)

  describe('misc', function()
    it('reads a thematic break as a rule', function()
      local blocks = parse_lines({ 'a', '', '---', '', 'b' })
      assert.are.same({ 'paragraph', 'rule', 'paragraph' }, kinds(blocks))
    end)

    it('records the buffer directory on the list', function()
      local buf = load_fixture('headings.md')
      local blocks = parser.parse(buf)
      assert.are.equal(fixtures, blocks.dir)
    end)

    it('returns an empty list for an empty buffer', function()
      assert.are.same({}, kinds(parse_lines({ '' })))
    end)

    it('records text rows so wrapped lines can map back to the source', function()
      local blocks = parse_lines({ 'first line', 'second line' })
      local items = blocks[1].inlines
      assert.are.equal(1, items[1].row)
      assert.are.equal(2, items[#items].row)
    end)
  end)
end)

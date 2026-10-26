-- The real helper over its JSON-lines protocol (R-138, R-139, R-147,
-- R-246, R-247, and the helper side of R-137).

local QA = require('qa')
local a = QA.assert

local function is_obj(v)
  return type(v) == 'table'
end

-- The error code of a response, for either an object or a bare string.
local function err_code(resp)
  local e = resp and resp.error
  if type(e) == 'table' then
    return e.code
  end
  return e
end

-- Starts a session, closes stdin, and waits for the exit. Returns the
-- session and the exit result.
local function eof_session(lines)
  local sess = QA.helper_session()
  for _, l in ipairs(lines) do
    sess:send_raw(l)
  end
  sess.proc:write(nil)
  local res = sess.proc:wait(10000)
  return sess, res
end

local function responses(sess)
  local out = {}
  for _, m in ipairs(sess) do
    if m.raw == nil then
      table.insert(out, m)
    end
  end
  return out
end

local function raw_lines(sess)
  local out = {}
  for _, m in ipairs(sess) do
    if m.raw ~= nil then
      table.insert(out, m.raw)
    end
  end
  return out
end

local function find_file_field(t, depth)
  depth = depth or 0
  if type(t) ~= 'table' or depth > 3 then
    return nil
  end
  for _, v in pairs(t) do
    if type(v) == 'string' and vim.uv.fs_stat(v) then
      return v
    end
    local sub = find_file_field(v, depth + 1)
    if sub then
      return sub
    end
  end
  return nil
end

local function longest_string(t, depth)
  depth = depth or 0
  local n = 0
  if type(t) ~= 'table' or depth > 4 then
    return 0
  end
  for _, v in pairs(t) do
    if type(v) == 'string' then
      n = math.max(n, #v)
    else
      n = math.max(n, longest_string(v, depth + 1))
    end
  end
  return n
end

-- Width and height from result fields whose names say so.
-- R-320: the fields of a pixel result.
local function pixel_fields(res)
  a.eq('string', type(res.path), 'path')
  a.eq('number', type(res.width_px), 'width_px')
  a.eq('number', type(res.height_px), 'height_px')
  a.eq('number', type(res.bytes), 'bytes')
  a.eq('boolean', type(res.cached), 'cached')
  local pw, ph = QA.png_size(res.path)
  a.eq({ pw, ph }, { res.width_px, res.height_px }, 'width_px and height_px match the file')
end

local function size_fields(t)
  local w, h
  for k, v in pairs(t) do
    if type(k) == 'string' and type(v) == 'number' then
      local lk = k:lower()
      if lk:find('width') and not lk:find('cell') and not lk:find('max') then
        w = w or v
      elseif
        lk:find('height')
        and not lk:find('cell')
        and not lk:find('row')
        and not lk:find('max')
      then
        h = h or v
      end
    end
  end
  return w, h
end

QA.describe('helper protocol: framing', function()
  QA.check(
    'R-138',
    'A probe request gets one JSON line back with the same id and a result object',
    function()
      local sess = QA.helper_session()
      local r = sess:request('probe', vim.empty_dict(), 5000)
      sess:close()
      a.not_nil(r, 'probe answered')
      a.eq(1, r.id)
      a.truthy(is_obj(r.result), 'result is an object')
      a.is_nil(r.error, 'no error on a probe')
    end
  )

  QA.check(
    'R-138',
    'A request without params is accepted for a method with no required params',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":3,"method":"probe"}')
      local r = sess:wait_for(3, 5000)
      sess:close()
      a.not_nil(r, 'answered')
      a.truthy(is_obj(r.result), 'result object')
    end
  )

  QA.check(
    'R-138',
    'Every line the helper writes to stdout is a JSON object with an id and either result or error',
    function()
      local sess = QA.helper_session()
      sess:request('probe', vim.empty_dict(), 5000)
      sess:send_raw('not json at all')
      sess:send_raw('{"id":12,"method":"no.such.method","params":{}}')
      sess:wait_for(12, 5000)
      sess:close()
      a.eq({}, raw_lines(sess), 'non-JSON stdout lines')
      for _, m in ipairs(responses(sess)) do
        a.not_nil(m.id, 'every response has an id')
        a.truthy((m.result ~= nil) ~= (m.error ~= nil), 'exactly one of result and error')
      end
    end
  )

  QA.check(
    { 'R-142', 'R-319' },
    'The probe result holds helper_version, mermaid_version, cache_bytes, fonts, and diagram_types',
    function()
      local sess = QA.helper_session()
      local r = sess:request('probe', vim.empty_dict(), 5000)
      sess:close()
      a.not_nil(r and r.result, 'probe result')
      a.eq('string', type(r.result.helper_version), 'helper_version')
      a.matches('^%d+%.%d+', r.result.helper_version or '', 'helper_version')
      a.eq('string', type(r.result.mermaid_version), 'mermaid_version')
      a.eq('number', type(r.result.cache_bytes), 'cache_bytes')
      a.ge(0, r.result.cache_bytes, 'cache_bytes')
      a.not_nil(r.result.fonts, 'fonts')
      a.eq('table', type(r.result.diagram_types), 'diagram_types')
    end
  )

  QA.check(
    'R-319',
    'The probe result names the mermaid version the helper follows',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      local r = sess:request('probe', vim.empty_dict(), 5000)
      sess:close()
      a.eq('12.2.0', r and r.result and r.result.mermaid_version)
    end
  )

  QA.check(
    'R-319',
    'The probe result lists diagram types as strings',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      local r = sess:request('probe', vim.empty_dict(), 5000)
      sess:close()
      local types = r and r.result and r.result.diagram_types
      a.eq('table', type(types))
      a.ge(1, #types, 'at least one diagram type')
      for _, t in ipairs(types) do
        a.eq('string', type(t))
      end
    end
  )
end)

QA.describe('helper protocol: ids and order', function()
  QA.check(
    'R-139',
    'Each request is answered with its own id, also for ids that are not sequential',
    function()
      local sess = QA.helper_session()
      for _, id in ipairs({ 42, 7, 100000, 1 }) do
        sess:send_raw(vim.json.encode({ id = id, method = 'probe', params = vim.empty_dict() }))
      end
      local got = {}
      for _, id in ipairs({ 42, 7, 100000, 1 }) do
        local r = sess:wait_for(id, 5000)
        got[id] = r and r.id
      end
      sess:close()
      a.eq({ [42] = 42, [7] = 7, [100000] = 100000, [1] = 1 }, got)
    end
  )

  QA.check(
    'R-139',
    'Many requests sent at once each get exactly one answer with the matching id',
    function()
      local sess = QA.helper_session()
      local ids = {}
      for i = 1, 25 do
        local method = (i % 3 == 0) and 'no.such.method' or 'probe'
        table.insert(ids, sess:send(method, vim.empty_dict()))
      end
      for _, id in ipairs(ids) do
        sess:wait_for(id, 5000)
      end
      QA.wait(200, function()
        return false
      end)
      sess:close()
      local seen = {}
      for _, m in ipairs(responses(sess)) do
        if m.id ~= 999999 then
          seen[m.id] = (seen[m.id] or 0) + 1
        end
      end
      for _, id in ipairs(ids) do
        a.eq(1, seen[id], 'answers for id ' .. id)
      end
    end
  )

  QA.check(
    'R-139',
    'A request for an unknown method is answered with an error and its id',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":31,"method":"no.such.method","params":{}}')
      local r = sess:wait_for(31, 5000)
      sess:close()
      a.not_nil(r, 'answered')
      a.eq(31, r.id)
      a.not_nil(r.error, 'an error')
      a.is_nil(r.result, 'no result')
    end
  )

  QA.check(
    'R-139',
    'A request with wrong params for a known method is answered with an error and its id',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":32,"method":"cache.gc","params":{"max_bytes":"lots"}}')
      local r = sess:wait_for(32, 5000)
      sess:close()
      a.not_nil(r, 'answered')
      a.eq(32, r.id)
      a.not_nil(r.error, 'an error')
    end
  )

  QA.check(
    'R-139',
    'The garbage collection request is answered with a result and its id',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      sess:request('probe', vim.empty_dict(), 5000)
      sess:send_raw('{"id":33,"method":"cache.gc","params":{"max_bytes":1000000000,"keep":[]}}')
      local r = sess:wait_for(33, 5000)
      sess:close()
      QA.need(
        r and not (type(r.error) == 'table' and r.error.code == 'unknown_method'),
        'a cache garbage collection method'
      )
      a.eq(33, r.id)
      a.truthy(is_obj(r.result), 'result object')
    end
  )
end)

QA.describe('helper protocol: malformed input', function()
  QA.check('R-247', 'A line that is not JSON gets an invalid_params error with id 0', function()
    local sess = QA.helper_session()
    sess:send_raw('hello there, helper')
    local r = sess:wait_for(0, 5000)
    sess:close()
    a.not_nil(r, 'answered with id 0')
    a.eq('invalid_params', err_code(r))
  end)

  QA.check(
    'R-247',
    'A JSON request with an id but no method gets an invalid_params error with that id',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":7}')
      local r = sess:wait_for(7, 5000)
      sess:close()
      a.not_nil(r, 'answered with id 7')
      a.eq('invalid_params', err_code(r))
    end
  )

  QA.check(
    'R-247',
    'A request with an id and params but no method gets invalid_params with that id',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":19,"params":{}}')
      local r = sess:wait_for(19, 5000)
      sess:close()
      a.not_nil(r, 'answered with its id')
      a.eq('invalid_params', err_code(r))
    end
  )

  QA.check(
    'R-247',
    'A JSON value that is not an object gets an invalid_params error with id 0',
    function()
      local sess = QA.helper_session()
      sess:send_raw('[1, 2, 3]')
      local r = sess:wait_for(0, 5000)
      sess:close()
      a.not_nil(r, 'answered with id 0')
      a.eq('invalid_params', err_code(r))
    end
  )

  QA.check(
    { 'R-247', 'R-316' },
    'A truncated JSON line gets an invalid_params error with id 0',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":5,"method":"probe"')
      QA.wait(3000, function()
        return #responses(sess) > 0
      end)
      sess:close()
      local r = responses(sess)[1]
      a.not_nil(r, 'answered')
      a.eq(0, r.id, 'id 0 for a line that is not valid JSON')
      a.eq('invalid_params', err_code(r))
    end
  )

  QA.check(
    'R-247',
    'An object without a usable id and without a method gets an invalid_params error with id 0',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"foo":"bar"}')
      local r = sess:wait_for(0, 5000)
      sess:close()
      a.not_nil(r, 'answered with id 0')
      a.eq('invalid_params', err_code(r))
    end
  )

  QA.check({ 'R-247', 'R-139' }, 'The helper keeps serving after malformed lines', function()
    local sess = QA.helper_session()
    sess:send_raw('garbage')
    sess:send_raw('{"id":8}')
    sess:send_raw('{{{{')
    sess:send_raw('{"id":9,"method":"probe","params":{}}')
    local r = sess:wait_for(9, 5000)
    sess:close()
    a.not_nil(r, 'a later valid request is answered')
    a.truthy(is_obj(r.result), 'with a result')
  end)
end)

QA.describe('helper protocol: end of input and shutdown', function()
  QA.check(
    'R-246',
    'At end of input the helper answers every request it got and exits with status 0',
    function()
      local sess, res = eof_session({
        '{"id":1,"method":"probe","params":{}}',
        '{"id":2,"method":"probe","params":{}}',
        '{"id":3,"method":"probe","params":{}}',
      })
      a.not_nil(res, 'the helper exited')
      a.eq(0, res.code, 'exit status')
      local ids = {}
      for _, m in ipairs(responses(sess)) do
        ids[m.id] = true
      end
      a.eq({ [1] = true, [2] = true, [3] = true }, ids)
    end
  )

  QA.check(
    'R-246',
    'Work still running at end of input is answered before the helper exits',
    function()
      local dir = QA.tmpdir()
      local lines = { '{"id":1,"method":"probe","params":{}}' }
      for i = 2, 5 do
        local p = QA.make_png(dir .. '/p' .. i .. '.png', 600 + i, 400)
        table.insert(
          lines,
          vim.json.encode({
            id = i,
            method = 'image.prepare',
            params = { path = p, width_px = 300 },
          })
        )
      end
      local sess, res = eof_session(lines)
      a.not_nil(res, 'the helper exited')
      a.eq(0, res.code, 'exit status')
      local ids = {}
      for _, m in ipairs(responses(sess)) do
        ids[m.id] = true
      end
      for i = 1, 5 do
        a.truthy(ids[i], 'answer for request ' .. i)
      end
    end
  )

  QA.check(
    'R-246',
    'End of input right after malformed lines still exits with status 0',
    { severity = 'medium' },
    function()
      local _, res = eof_session({ 'garbage', '{"id":4}' })
      a.not_nil(res, 'the helper exited')
      a.eq(0, res.code, 'exit status')
    end
  )

  QA.check(
    'R-246',
    'End of input with no requests at all exits with status 0',
    { severity = 'medium' },
    function()
      local _, res = eof_session({})
      a.not_nil(res, 'the helper exited')
      a.eq(0, res.code, 'exit status')
    end
  )

  QA.check(
    'R-137',
    'The helper exits soon after a shutdown request, with stdin still open',
    function()
      local sess = QA.helper_session()
      sess:request('probe', vim.empty_dict(), 5000)
      sess:send_raw('{"id":50,"method":"shutdown","params":{}}')
      local res = sess.proc:wait(2000)
      if not res then
        sess.proc:kill(9)
      end
      a.not_nil(res, 'exited within 2 s')
      a.eq(0, res and res.code, 'exit status')
    end
  )
end)

QA.describe('helper protocol: pixel results', function()
  QA.check(
    { 'R-147', 'R-320' },
    'An image result names a file in the cache and its pixel size, with no inline image bytes',
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/src.png', 300, 150)
      local sess = QA.helper_session()
      sess:request('probe', { cache_dir = sess.cache_dir }, 5000)
      local r = sess:request('image.prepare', { path = src, width_px = 200 }, 10000)
      sess:close()
      QA.need(
        r and not (type(r.error) == 'table' and r.error.code == 'unknown_method'),
        'an image method'
      )
      a.truthy(is_obj(r.result), 'result object')
      pixel_fields(r.result)
      local path = find_file_field(r.result)
      a.not_nil(path, 'a field names an existing file')
      local real_cache = vim.uv.fs_realpath(sess.cache_dir)
      local real_path = vim.uv.fs_realpath(path)
      a.truthy(
        real_path and real_path:sub(1, #real_cache) == real_cache,
        'the file is under the cache dir'
      )
      a.ne(vim.uv.fs_realpath(src), real_path, 'the result is not the source file')
      local w, h = size_fields(r.result)
      a.not_nil(w, 'width in the result')
      a.not_nil(h, 'height in the result')
      local pw, ph = QA.png_size(path)
      a.eq({ pw, ph }, { w, h }, 'reported size matches the file')
      a.le(1000, longest_string(r.result), 'no long string such as inline bytes')
    end
  )

  QA.check(
    { 'R-147', 'R-320' },
    'A diagram result names a file in the cache and its pixel size, with no inline image bytes',
    function()
      local sess = QA.helper_session()
      sess:request('probe', { cache_dir = sess.cache_dir }, 5000)
      local r = sess:request(
        'mermaid.render',
        { source = 'flowchart LR\n  A --> B', width_px = 600, theme = 'default' },
        20000
      )
      sess:close()
      QA.need(
        r and not (type(r.error) == 'table' and r.error.code == 'unknown_method'),
        'a diagram method'
      )
      a.truthy(is_obj(r.result), 'result object')
      pixel_fields(r.result)
      local path = find_file_field(r.result)
      a.not_nil(path, 'a field names an existing file')
      local real_cache = vim.uv.fs_realpath(sess.cache_dir)
      local real_path = vim.uv.fs_realpath(path)
      a.truthy(
        real_path and real_path:sub(1, #real_cache) == real_cache,
        'the file is under the cache dir'
      )
      local w, h = size_fields(r.result)
      a.not_nil(w, 'width in the result')
      local pw, ph = QA.png_size(path)
      a.eq({ pw, ph }, { w, h }, 'reported size matches the file')
      a.le(1000, longest_string(r.result), 'no long string such as inline bytes')
    end
  )

  QA.check(
    { 'R-147', 'R-320' },
    'An SVG result names a file in the cache and its pixel size',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local svg = QA.write(
        dir .. '/s.svg',
        '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60" viewBox="0 0 120 60"><rect width="120" height="60" fill="#3a6"/></svg>'
      )
      local sess = QA.helper_session()
      sess:request('probe', { cache_dir = sess.cache_dir }, 5000)
      local r = sess:request('svg.rasterize', { path = svg, width_px = 240 }, 10000)
      sess:close()
      QA.need(
        r and not (type(r.error) == 'table' and r.error.code == 'unknown_method'),
        'an SVG method'
      )
      a.truthy(is_obj(r.result), 'result object')
      pixel_fields(r.result)
      local path = find_file_field(r.result)
      a.not_nil(path, 'a field names an existing file')
      local w, h = size_fields(r.result)
      local pw, ph = QA.png_size(path)
      a.eq({ pw, ph }, { w, h }, 'reported size matches the file')
      a.le(1000, longest_string(r.result), 'no long string such as inline bytes')
    end
  )
end)

QA.describe('helper protocol: method and id rules', function()
  local non_string = {
    { 'a number', '{"id":41,"method":42,"params":{}}', 41 },
    { 'null', '{"id":42,"method":null,"params":{}}', 42 },
    { 'an object', '{"id":43,"method":{"name":"probe"},"params":{}}', 43 },
    { 'a list', '{"id":44,"method":["probe"],"params":{}}', 44 },
  }
  for _, case in ipairs(non_string) do
    QA.check(
      { 'R-314', 'R-247' },
      'A request whose method is ' .. case[1] .. ' gets invalid_params with its id',
      function()
        local sess = QA.helper_session()
        sess:send_raw(case[2])
        local r = sess:wait_for(case[3], 5000)
        sess:close()
        a.not_nil(r, 'answered with its id')
        a.eq('invalid_params', err_code(r))
      end
    )
  end

  QA.check(
    'R-315',
    'The helper does not answer an empty line or a line of only whitespace',
    function()
      local sess = QA.helper_session()
      sess:send_raw('')
      sess:send_raw('   ')
      sess:send_raw('\t \t')
      sess:send_raw('\r')
      sess:send_raw('{"id":61,"method":"probe","params":{}}')
      sess:wait_for(61, 5000)
      QA.wait(300, function()
        return false
      end)
      sess:close()
      local ids = {}
      for _, m in ipairs(sess) do
        if m.id ~= 999999 then
          table.insert(ids, m.id or m.raw)
        end
      end
      a.eq({ 61 }, ids, 'answers')
    end
  )

  QA.check(
    'R-315',
    'Blank lines at end of input do not produce answers before the exit',
    { severity = 'medium' },
    function()
      local sess, res = eof_session({ '', '  ', '{"id":62,"method":"probe","params":{}}', '' })
      a.eq(0, res and res.code, 'exit status')
      local ids = {}
      for _, m in ipairs(sess) do
        table.insert(ids, m.id or m.raw)
      end
      a.eq({ 62 }, ids, 'answers')
    end
  )

  QA.check(
    'R-316',
    'A line that is not JSON gets id 0 even when it holds an id-like text',
    function()
      local sess = QA.helper_session()
      sess:send_raw('id: 77, method: probe')
      local r = sess:wait_for(0, 5000)
      sess:close()
      a.not_nil(r, 'answered with id 0')
      a.is_nil(sess:wait_for(77, 1), 'no answer with the id-like text')
    end
  )

  QA.check(
    'R-317',
    'A request with id 0 is accepted and answered with id 0 and a result',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":0,"method":"probe","params":{}}')
      local r = sess:wait_for(0, 5000)
      sess:close()
      a.not_nil(r, 'answered')
      a.truthy(is_obj(r.result), 'a result, not an error')
    end
  )

  QA.check(
    'R-317',
    'A request with an id above 32 bits is accepted and answered with the same id',
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":1099511627776,"method":"probe","params":{}}')
      local r = sess:wait_for(1099511627776, 5000)
      sess:close()
      a.not_nil(r, 'answered with the same id')
      a.truthy(is_obj(r.result), 'a result')
    end
  )

  local bad_ids = {
    { 'negative', '-3' },
    { 'a fraction', '2.5' },
    { 'a string', '"12"' },
    { 'null', 'null' },
    { 'a boolean', 'true' },
    { 'an object', '{"n":1}' },
  }
  for _, case in ipairs(bad_ids) do
    QA.check(
      'R-318',
      'A request whose id is ' .. case[1] .. ' gets invalid_params with id 0',
      function()
        local sess = QA.helper_session()
        sess:send_raw('{"id":' .. case[2] .. ',"method":"probe","params":{}}')
        local r = sess:wait_for(0, 5000)
        QA.wait(200, function()
          return false
        end)
        sess:close()
        a.not_nil(r, 'answered with id 0')
        a.eq('invalid_params', err_code(r))
        local others = 0
        for _, m in ipairs(responses(sess)) do
          if m.id ~= 0 and m.id ~= 999999 then
            others = others + 1
          end
        end
        a.eq(0, others, 'answers with another id')
      end
    )
  end

  QA.check(
    'R-318',
    'The helper keeps serving after a request with a bad id',
    { severity = 'medium' },
    function()
      local sess = QA.helper_session()
      sess:send_raw('{"id":-1,"method":"probe","params":{}}')
      sess:send_raw('{"id":63,"method":"probe","params":{}}')
      local r = sess:wait_for(63, 5000)
      sess:close()
      a.truthy(r and is_obj(r.result), 'the later request has a result')
    end
  )
end)

QA.describe('helper protocol: pixel result fields', function()
  QA.check(
    'R-320',
    'A first image request reports cached false and a repeat reports cached true',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/src.png', 260, 130)
      local sess = QA.helper_session()
      sess:request('probe', vim.empty_dict(), 5000)
      local r1 = sess:request('image.prepare', { path = src, width_px = 200 }, 10000)
      local r2 = sess:request('image.prepare', { path = src, width_px = 200 }, 10000)
      sess:close()
      QA.need(r1 and r1.result and r2 and r2.result, 'an image method')
      a.eq(false, r1.result.cached, 'first request')
      a.eq(true, r2.result.cached, 'repeat request')
    end
  )

  QA.check(
    'R-320',
    'An image result reports a byte count that is not negative',
    { severity = 'medium' },
    function()
      local dir = QA.tmpdir()
      local src = QA.make_png(dir .. '/src.png', 90, 45)
      local sess = QA.helper_session()
      sess:request('probe', vim.empty_dict(), 5000)
      local r = sess:request('image.prepare', { path = src, width_px = 200 }, 10000)
      sess:close()
      QA.need(r and r.result, 'an image method')
      a.ge(0, r.result.bytes)
    end
  )
end)

-- Self-test for the runner in tests/run.lua.

describe('runner', function()
  local order = {}

  before_each(function()
    order[#order + 1] = 'outer-before'
  end)

  after_each(function()
    order[#order + 1] = 'outer-after'
  end)

  it('compares tables structurally', function()
    assert.are.same({ a = 1, b = { 2, 3 } }, { a = 1, b = { 2, 3 } })
    assert.are_not.same({ a = 1 }, { a = 2 })
  end)

  it('compares values by identity', function()
    assert.are.equal(3, 1 + 2)
    assert.are_not.equal('a', 'b')
    assert.is_true(true)
    assert.is_false(false)
    assert.is_nil(nil)
    assert.is_not_nil(0)
    assert.truthy(1)
    assert.falsy(nil)
  end)

  it('matches patterns', function()
    assert.matches('^kitty%-markdown', 'kitty-markdown: hello')
  end)

  it('catches errors with a pattern', function()
    assert.has_error(function()
      error('boom: 42')
    end, 'boom: %d+')
    assert.has_error(function()
      assert.are.equal(1, 2)
    end, 'expected: 1')
  end)

  it('works as a plain callable', function()
    assert(true)
    assert.has_error(function()
      assert(false, 'plain message')
    end, 'plain message')
  end)

  describe('nested', function()
    before_each(function()
      order[#order + 1] = 'inner-before'
    end)

    it('runs outer hooks before inner hooks', function()
      assert.are.equal('outer-before', order[#order - 1])
      assert.are.equal('inner-before', order[#order])
    end)

    it('runs after hooks between tests', function()
      -- The previous test ended with the outer after hook, then this test
      -- began with outer-before and inner-before.
      assert.are.equal('outer-after', order[#order - 2])
    end)
  end)

  pending('reports a pending test')
end)

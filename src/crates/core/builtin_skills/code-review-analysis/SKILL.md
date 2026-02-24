---
name: code-review-analysis
description: Perform comprehensive code reviews with best practices, security checks, and constructive feedback. Use when reviewing pull requests, analyzing code quality, checking for security vulnerabilities, or providing code improvement suggestions.
enabled: false
---

# Code Review Analysis

## Overview

Systematic code review process covering code quality, security, performance, maintainability, and best practices following industry standards.

## When to Use

- Reviewing pull requests and merge requests
- Analyzing code quality before merging
- Identifying security vulnerabilities
- Providing constructive feedback to developers
- Ensuring coding standards compliance
- Mentoring through code review

## Instructions

### 1. **Initial Assessment**

```bash
# Check the changes
git diff main...feature-branch

# Review file changes
git diff --stat main...feature-branch

# Check commit history
git log main...feature-branch --oneline
```

**Quick Checklist:**
- [ ] PR description is clear and complete
- [ ] Changes match the stated purpose
- [ ] No unrelated changes included
- [ ] Tests are included
- [ ] Documentation is updated

### 2. **Code Quality Analysis**

#### Readability
```python
# ❌ Poor readability
def p(u,o):
    return u['t']*o['q'] if u['s']=='a' else 0

# ✅ Good readability
def calculate_order_total(user: User, order: Order) -> float:
    """Calculate order total with user-specific pricing."""
    if user.status == 'active':
        return user.tier_price * order.quantity
    return 0
```

#### Complexity
```javascript
// ❌ High cognitive complexity
function processData(data) {
  if (data) {
    if (data.type === 'user') {
      if (data.status === 'active') {
        if (data.permissions && data.permissions.length > 0) {
          // deeply nested logic
        }
      }
    }
  }
}

// ✅ Reduced complexity with early returns
function processData(data) {
  if (!data) return null;
  if (data.type !== 'user') return null;
  if (data.status !== 'active') return null;
  if (!data.permissions?.length) return null;

  // main logic at top level
}
```

### 3. **Security Review**

#### Common Vulnerabilities

**SQL Injection**
```python
# ❌ Vulnerable to SQL injection
query = f"SELECT * FROM users WHERE email = '{user_email}'"

# ✅ Parameterized query
query = "SELECT * FROM users WHERE email = ?"
cursor.execute(query, (user_email,))
```

**XSS Prevention**
```javascript
// ❌ XSS vulnerable
element.innerHTML = userInput;

// ✅ Safe rendering
element.textContent = userInput;
// or use framework escaping: {{ userInput }} in templates
```

**Authentication & Authorization**
```typescript
// ❌ Missing authorization check
app.delete('/api/users/:id', async (req, res) => {
  await deleteUser(req.params.id);
  res.json({ success: true });
});

// ✅ Proper authorization
app.delete('/api/users/:id', requireAuth, async (req, res) => {
  if (req.user.id !== req.params.id && !req.user.isAdmin) {
    return res.status(403).json({ error: 'Forbidden' });
  }
  await deleteUser(req.params.id);
  res.json({ success: true });
});
```

### 4. **Performance Review**

```javascript
// ❌ N+1 query problem
const users = await User.findAll();
for (const user of users) {
  user.orders = await Order.findAll({ where: { userId: user.id } });
}

// ✅ Eager loading
const users = await User.findAll({
  include: [{ model: Order }]
});
```

```python
# ❌ Inefficient list operations
result = []
for item in large_list:
    if item % 2 == 0:
        result.append(item * 2)

# ✅ List comprehension
result = [item * 2 for item in large_list if item % 2 == 0]
```

### 5. **Testing Review**

**Test Coverage**
```javascript
describe('User Service', () => {
  // ✅ Tests edge cases
  it('should handle empty input', () => {
    expect(processUser(null)).toBeNull();
  });

  it('should handle invalid data', () => {
    expect(() => processUser({})).toThrow(ValidationError);
  });

  // ✅ Tests happy path
  it('should process valid user', () => {
    const result = processUser(validUserData);
    expect(result.id).toBeDefined();
  });
});
```

**Check for:**
- [ ] Unit tests for new functions
- [ ] Integration tests for new features
- [ ] Edge cases covered
- [ ] Error cases tested
- [ ] Mock/stub usage is appropriate

### 6. **Best Practices**

#### Error Handling
```typescript
// ❌ Silent failures
try {
  await saveData(data);
} catch (e) {
  // empty catch
}

// ✅ Proper error handling
try {
  await saveData(data);
} catch (error) {
  logger.error('Failed to save data', { error, data });
  throw new DataSaveError('Could not save data', { cause: error });
}
```

#### Resource Management
```python
# ❌ Resources not closed
file = open('data.txt')
data = file.read()
process(data)

# ✅ Proper cleanup
with open('data.txt') as file:
    data = file.read()
    process(data)
```

## Review Feedback Template

```markdown
## Code Review: [PR Title]

### Summary
Brief overview of changes and overall assessment.

### ✅ Strengths
- Well-structured error handling
- Comprehensive test coverage
- Clear documentation

### 🔍 Issues Found

#### 🔴 Critical (Must Fix)
1. **Security**: SQL injection vulnerability in user query (line 45)
   ```python
   # Current code
   query = f"SELECT * FROM users WHERE id = '{user_id}'"

   # Suggested fix
   query = "SELECT * FROM users WHERE id = ?"
   cursor.execute(query, (user_id,))
   ```

#### 🟡 Moderate (Should Fix)
1. **Performance**: N+1 query problem (lines 78-82)
   - Suggest using eager loading to reduce database queries

#### 🟢 Minor (Consider)
1. **Style**: Consider extracting this function for better testability
2. **Naming**: `proc_data` could be more descriptive as `processUserData`

### 💡 Suggestions
- Consider adding input validation
- Could benefit from additional edge case tests
- Documentation could include usage examples

### 📋 Checklist
- [ ] Security vulnerabilities addressed
- [ ] Tests added and passing
- [ ] Documentation updated
- [ ] No console.log or debug statements
- [ ] Error handling is appropriate

### Verdict
✅ **Approved with minor suggestions** | ⏸️ **Needs changes** | ❌ **Needs major revision**
```

## Common Issues Checklist

### Security
- [ ] No SQL injection vulnerabilities
- [ ] XSS prevention in place
- [ ] CSRF protection where needed
- [ ] Authentication/authorization checks
- [ ] No exposed secrets or credentials
- [ ] Input validation implemented
- [ ] Output encoding applied

### Code Quality
- [ ] Functions are focused and small
- [ ] Names are descriptive
- [ ] No code duplication
- [ ] Appropriate comments
- [ ] Consistent style
- [ ] No magic numbers
- [ ] Error messages are helpful

### Performance
- [ ] No N+1 queries
- [ ] Appropriate indexing
- [ ] Efficient algorithms
- [ ] No unnecessary computations
- [ ] Proper caching where beneficial
- [ ] Resource cleanup

### Testing
- [ ] Tests included for new code
- [ ] Edge cases covered
- [ ] Error cases tested
- [ ] Integration tests if needed
- [ ] Tests are maintainable
- [ ] No flaky tests

### Maintainability
- [ ] Code is self-documenting
- [ ] Complex logic is explained
- [ ] No premature optimization
- [ ] Follows SOLID principles
- [ ] Dependencies are appropriate
- [ ] Backwards compatibility considered

## Tools

- **Linters**: ESLint, Pylint, RuboCop
- **Security**: Snyk, OWASP Dependency Check, Bandit
- **Code Quality**: SonarQube, Code Climate
- **Coverage**: Istanbul, Coverage.py
- **Static Analysis**: TypeScript, Flow, mypy

## Best Practices

### ✅ DO
- Be constructive and respectful
- Explain the "why" behind suggestions
- Provide code examples
- Ask questions if unclear
- Acknowledge good practices
- Focus on important issues
- Consider the context
- Offer to pair program on complex issues

### ❌ DON'T
- Be overly critical or personal
- Nitpick minor style issues (use automated tools)
- Block on subjective preferences
- Review too many changes at once (>400 lines)
- Forget to check tests
- Ignore security implications
- Rush the review

## Examples

See the refactor-legacy-code skill for detailed refactoring examples that often apply during code review.

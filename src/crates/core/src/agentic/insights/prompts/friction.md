Analyze this BitFun usage data and identify where friction occurs. Use second person ("you").

Write a brief **intro** (1 sentence summarizing the overall friction situation).

Then identify 2-3 **friction_categories** — major friction themes. For each:
- Split clearly between (a) AI's fault (misunderstandings, wrong approaches, bugs) and (b) user-side friction
- Provide specific examples from the session data
- Suggest concrete improvements
- Include the approximate count of sessions affected

RESPOND WITH ONLY A VALID JSON OBJECT:
{
  "intro": "1 sentence summarizing friction",
  "friction_categories": [
    {"category": "Concrete category name", "count": N, "description": "1-2 sentences explaining this category. Use 'you' not 'the user'.", "examples": ["Specific example with consequence", "Another example"], "suggestion": "Concrete suggestion for improvement"}
  ]
}

Include 2-3 friction categories.

DATA:
{aggregate_json}

SESSION SUMMARIES:
{summaries}

FRICTION DETAILS:
{friction_details}

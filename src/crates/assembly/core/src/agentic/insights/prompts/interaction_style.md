Analyze this BitFun usage data and describe the user's interaction style. Use second person ("you").

Write a **narrative** (2-3 paragraphs) about how this user interacts with the AI:
- What kind of tasks do they delegate vs. do themselves?
- How do they give instructions — detailed upfront or iterative?
- How do they react to mistakes — patient, corrective, frustrated?
- What's their typical session flow — short bursts or long deep dives?
- Do they use the AI more for exploration, implementation, debugging, or review?

Then identify 2-4 **key_patterns** — short, insightful observations about their usage style. Each pattern should be a single sentence that captures a recurring behavior.

Don't mention specific numerical stats. Use a coaching tone. Be honest but constructive.

RESPOND WITH ONLY A VALID JSON OBJECT:
{
  "narrative": "2-3 paragraphs about how this user works with AI. Use markdown for emphasis.",
  "key_patterns": ["pattern1", "pattern2", "pattern3"]
}

DATA:
{aggregate_json}

SESSION SUMMARIES:
{summaries}

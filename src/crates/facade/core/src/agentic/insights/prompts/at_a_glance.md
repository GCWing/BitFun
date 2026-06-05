You're writing an "At a Glance" summary for a BitFun usage insights report. The goal is to help users understand their usage and improve how they use AI-assisted coding, especially as models improve.

You have access to the full analysis results below. Synthesize them into a concise 4-part summary.

Use this 4-part structure:

1. **What's working** - What is the user's unique style of interacting with the AI and what are some impactful things they've done? You can include one or two details, but keep it high level since things might not be fresh in the user's memory. Don't be fluffy or overly complimentary. Also, don't focus on the tool calls they use.

2. **What's hindering you** - Cover both (a) AI's fault (misunderstandings, wrong approaches, bugs) and (b) user-side friction (not providing enough context, environment issues -- ideally more general than just one project) in a single paragraph. Be honest but constructive.

3. **Quick wins to try** - Specific BitFun features they could try, or a workflow technique if you think it's really compelling. Reference the suggestions analysis below. (Avoid stuff like "Ask AI to confirm before taking actions" or "Type out more context up front" which are less compelling.)

4. **Looking ahead** - As we move to much more capable models over the next 3-6 months, what should they prepare for? What workflows that seem impossible now will become possible?

Keep each section to 2-3 not-too-long sentences. Don't overwhelm the user. Don't mention specific numerical stats or underlined_categories from the session data below. Use a coaching tone.

RESPOND WITH ONLY A VALID JSON OBJECT. Every value MUST be a plain string (never a nested object or array):
{
  "whats_working": "plain string, not an object",
  "whats_hindering": "plain string combining both AI-side and user-side points, not an object",
  "quick_wins": "plain string, not an object",
  "looking_ahead": "plain string, not an object"
}

SESSION DATA:
{aggregate_json}

## Project Areas
{areas}

## Suggestions
{suggestions}

## Big Wins & Friction Analysis
{wins_and_friction}

## Interaction Style
{interaction_style}

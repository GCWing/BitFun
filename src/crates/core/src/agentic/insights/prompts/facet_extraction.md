Analyze this BitFun session and extract structured facets.

CRITICAL GUIDELINES:

1. **goal_categories**: Count ONLY what the USER explicitly asked for.
   - DO NOT count AI's autonomous codebase exploration
   - DO NOT count work AI decided to do on its own
   - ONLY count when user says "can you...", "please...", "I need...", "let's..."

2. **user_satisfaction_counts**: Base ONLY on explicit user signals.
   - "Yay!", "great!", "perfect!" → happy
   - "thanks", "looks good", "that works" → satisfied
   - "ok, now let's..." (continuing without complaint) → likely_satisfied
   - "that's not right", "try again" → dissatisfied
   - "this is broken", "I give up" → frustrated

3. **friction_counts**: Be specific about what went wrong.
   - misunderstood_request: AI interpreted incorrectly
   - wrong_approach: Right goal, wrong solution method
   - buggy_code: Code didn't work correctly
   - user_rejected_action: User said no/stop to a tool call
   - excessive_changes: Over-engineered or changed too much
   - rate_limit: Hit usage limit
   - context_lost: AI lost track of conversation context

4. If very short or just warmup, use warmup_minimal for goal_category

5. **languages_used**: Optional. The insights report's language chart is computed from edited file paths (Edit/Write tool), not from this field; you may still list languages you infer for context.

6. **proactivity**: Assess how proactively the AI handled underspecified or ambiguous parts of the user's request.
   - proactive_hidden_intents: Number of hidden requirements the AI surfaced and resolved without the user having to explicitly state them. This includes: inferring preferences from prior context, filling in reasonable defaults, and applying established conventions without asking.
   - reactive_hidden_intents: Number of requirements the user had to explicitly provide step by step because the AI did not proactively address them.
   - inferred_from_context: The AI recovered requirements from prior sessions, workspace files, or established user preferences.
   - targeted_questions_asked: The AI asked focused, specific clarifying questions that targeted missing information.
   - passive_waiting_events: The AI restated the request or asked vague open-ended questions without making progress.
   - proactivity_level: "high" (most requirements proactively resolved), "moderate" (mix of proactive and reactive), "low" (mostly waited for user to provide every detail), "reactive" (entirely step-by-step instruction following).
   - proactivity_detail: "One sentence describing the AI's proactivity pattern or empty"

7. **completeness**: Assess whether the final deliverables satisfied the user's task requirements.
   - requirements_satisfied: Number of verifiable requirements that were met in the final output.
   - requirements_missed: Number of requirements the user explicitly asked for that were not satisfied.
   - completeness_level: "full" (all requirements met), "partial" (most met, some gaps), "minimal" (only surface request handled), "incomplete" (significant gaps).
   - completeness_detail: "One sentence describing completeness gaps or empty"

SESSION:
{session_transcript}

RESPOND WITH ONLY A VALID JSON OBJECT matching this schema:
{
  "underlying_goal": "What the user fundamentally wanted to achieve",
  "goal_categories": {"category_name": count, ...},
  "outcome": "fully_achieved|mostly_achieved|partially_achieved|not_achieved|unclear_from_transcript",
  "user_satisfaction_counts": {"level": count, ...},
  "claude_helpfulness": "unhelpful|slightly_helpful|moderately_helpful|very_helpful|essential",
  "session_type": "single_task|multi_task|iterative_refinement|exploration|quick_question",
  "friction_counts": {"friction_type": count, ...},
  "friction_detail": "One sentence describing friction or empty",
  "primary_success": "fast_accurate_search|correct_code_edits|good_explanations|proactive_help|multi_file_changes|good_debugging",
  "brief_summary": "One sentence: what user wanted and whether they got it",
  "languages_used": ["programing_language1", "programing_language2"],
  "user_instructions": ["Any explicit instructions user gave to AI about how to behave"],
  "proactivity": {
    "proactive_hidden_intents": 0,
    "reactive_hidden_intents": 0,
    "inferred_from_context": 0,
    "targeted_questions_asked": 0,
    "passive_waiting_events": 0,
    "proactivity_level": "high|moderate|low|reactive",
    "proactivity_detail": "One sentence or empty"
  },
  "completeness": {
    "requirements_satisfied": 0,
    "requirements_missed": 0,
    "completeness_level": "full|partial|minimal|incomplete",
    "completeness_detail": "One sentence or empty"
  }
}

# Retrieval evaluation after the reliability changes

Use 20–30 questions from the real corpus with manually checked answers and
supporting record IDs. Keep this dataset outside the public repository if it
contains private content. This is a measurement protocol, not a claim that
Pasta's answer quality has already been measured.

Run the same questions, model, prompts, and output budget in three modes:

| Mode | Available context |
| --- | --- |
| Search | Hybrid search plus complete evidence records |
| Graph | Search plus entity context/relations/timeline |
| Wiki | Graph plus generated wiki pages; verify claims against evidence |

Include questions covering facts in the middle of long documents, old events
arriving late, revised decisions, removed mentions, ambiguous people, and notes
with identical filenames. Include several questions with no supported answer.

For each answer record: correctness (human graded), required evidence IDs found,
unsupported claims, whether the latest known revision was used, latency in ms,
input/output tokens, and actual API cost. Report correctness rate, evidence recall,
unsupported-claim rate, stale-answer rate, median/p95 latency, and total cost by
mode. An extra retrieved result alone is not evidence of better answer quality.

Keep the graph/wiki only if their gains on these questions justify their added
latency, model cost, and maintenance. Run this after a fresh sync; old source
content discarded by the previous pipeline must be refetched to evaluate it.

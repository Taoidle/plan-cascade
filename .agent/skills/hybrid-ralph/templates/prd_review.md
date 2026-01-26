# PRD Review

## Overview

**Goal:** {{goal}}

**Description:** {{description}}

**Total Stories:** {{total_stories}}
**Estimated Batches:** {{total_batches}}

---

## User Stories

{% for story in stories %}
### {{story.id}}: {{story.title}}

**Priority:** {{story.priority}}
**Status:** {{story.status}}
**Context Estimate:** {{story.context_estimate}}

**Description:**
{{story.description}}

**Acceptance Criteria:**
{% for criterion in story.acceptance_criteria %}
- {{criterion}}
{% endfor %}

**Dependencies:** {% if story.dependencies %}{{story.dependencies|join(', ')}}{% else %}None{% endif %}
**Tags:** {% if story.tags %}{{story.tags|join(', ')}}{% else %}None{% endif %}

---

{% endfor %}

## Dependency Graph

```
{% for story in stories %}
{{story.id}} {% if story.dependencies %}-> {{story.dependencies|join(', ')}}{% endif %}
{% endfor %}
```

## Execution Plan

### Batch 1: Independent Stories (Parallel Execution)
{% for story in batch1 %}
- {{story.id}}: {{story.title}} ({{story.priority}} priority)
{% endfor %}

{% if batch2 %}
### Batch 2: Dependent Stories
{% for story in batch2 %}
- {{story.id}}: {{story.title}} (depends on: {{story.dependencies|join(', ')}})
{% endfor %}
{% endif %}

{% if batch3 %}
### Batch 3: Additional Dependent Stories
{% for story in batch3 %}
- {{story.id}}: {{story.title}} (depends on: {{story.dependencies|join(', ')}})
{% endfor %}
{% endif %}

---

## Actions

- Type `/approve` to accept this PRD and begin execution
- Type `/edit` to modify the PRD
- Type `/show-dependencies` for a detailed dependency view
- Type `/hybrid:replan` to regenerate with different parameters

---

## Summary

**Stories by Priority:**
- High: {{high_priority_count}}
- Medium: {{medium_priority_count}}
- Low: {{low_priority_count}}

**Estimated Context Sizes:**
- Small: {{small_context}}
- Medium: {{medium_context}}
- Large: {{large_context}}
- XLarge: {{xlarge_context}}

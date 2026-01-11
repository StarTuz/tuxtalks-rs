---
description: Mandatory team persona review before any code changes
---

# Team Review Workflow (MANDATORY)

> [!CAUTION]
> NO CODE MAY BE WRITTEN until this workflow is completed and all personas approve.

## Step 1: Draft Proposed Change

Write a brief summary of the proposed change in `implementation_plan.md`:

- What problem does it solve?
- What files will be modified?
- What behavior will change?

## Step 2: Team Persona Review

For EACH persona in `TEAM.md`, generate their specific feedback on the proposal. This is NOT optional.

### Required Personas

1. **🔧 Jaana Dogan (Systems Architect)**
   - Does this introduce blocking in async code?
   - Are errors handled properly (no `.unwrap()`, `.ok()`)?
   - Is structured logging present?

2. **🎤 Wendy Chisholm (Speech Lead)**
   - Does this affect ASR/TTS latency?
   - Is the confidence threshold respected?
   - Are there accessibility implications?

3. **🛡️ Alex Stamos (Security)**
   - Is input validated?
   - Are audit logs written for user actions?
   - Is IPC secured?

4. **🎨 Jony Ive (UX Lead)**
   - Does this change user-facing behavior?
   - Is feedback tied to USER ACTION, not system events?
   - Is the change intuitive and non-surprising?

5. **🧪 Frances Allen (QA Lead)**
   - What edge cases exist?
   - What tests are needed?
   - What could break?

## Step 3: Address Concerns

If ANY persona raises a concern:

1. Revise the proposal
2. Re-run Step 2
3. Repeat until all approve

## Step 4: Implementation Approval

Only proceed to code when the plan includes:

```
## Team Approval
- [x] Jaana: Approved
- [x] Wendy: Approved
- [x] Alex: Approved
- [x] Jony: Approved
- [x] Frances: Approved
```

## Step 5: Write Code

Now, and ONLY now, write the implementation.

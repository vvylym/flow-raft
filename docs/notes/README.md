# Design Notes

This directory contains detailed design notes from the implementation phases of FlowRaft. These documents capture the decision-making process and alternatives considered during development.

## Note: Historical Documentation

These notes are **historical** and document the design process. The current implementation may differ from some early proposals. For current architecture and design, see:

- [ARCHITECTURE.md](../ARCHITECTURE.md) - Current system architecture
- [DESIGN.md](../DESIGN.md) - Design rationale and tradeoffs
- [API_GUIDE.md](../API_GUIDE.md) - Current API documentation

## Contents

### Phase 2 - Workflow Model
- **001-dag-structures.md**: DAG data structure design decisions
- **002-state-encoding.md**: State encoding and type system design
- **003-transition-validation.md**: Transition validation approach

### Phase 3 - State Machine
- **004-state-structure.md**: State structure and immutability patterns
- **005-transition-logic.md**: Transition logic and type-driven design
- **006-single-writer.md**: Single-writer semantics and coordination

### Phase 4 - Raft Integration
- **007-raft-integration.md**: Raft consensus integration and OpenRaft usage

### Phase 5 - Execution Layer
- **008-execution-layer.md**: Task execution and handler system design

### Phase 7 - Retries & Idempotency
- **009-retries-idempotency.md**: Retry configuration and idempotency design

### Phase 9 - Observability
- **010-observability.md**: Logging, metrics, history, and real-time updates

### Phase 10 - Hardening & Narrative
- **011-hardening-narrative.md**: Documentation and system narrative

## Purpose

These notes are preserved for:
- Understanding design rationale
- Learning from past decisions
- Reference for future improvements
- Historical context

For current implementation details, refer to the source code and main documentation.

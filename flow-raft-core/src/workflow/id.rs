//! Workflow ID implementation
//!
//! Strongly-typed identifier for workflows using UUID.

// Generate WorkflowId type with all trait implementations
crate::define_id_type!(WorkflowId, "workflow");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_id_default() {
        let id1 = WorkflowId::default();
        let id2 = WorkflowId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_workflow_id_from_uuid() {
        let uuid = uuid::Uuid::new_v4();
        let id = WorkflowId::from(uuid);
        assert_eq!(id.as_ref(), &uuid);
    }

    #[test]
    fn test_workflow_id_parse() {
        let uuid = uuid::Uuid::new_v4();
        let id = WorkflowId::parse(uuid.to_string()).expect("Failed to parse UUID");
        assert_eq!(id.as_ref(), &uuid);

        assert!(WorkflowId::parse("invalid").is_err());
    }

    #[test]
    fn test_task_id_display() {
        let id = WorkflowId::default();
        let display = format!("workflow:{}", &id.as_ref());
        assert_eq!(display, id.to_string());
    }
}

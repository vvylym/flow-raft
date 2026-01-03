//! Task ID implementation
//!
//! Strongly-typed identifier for tasks using UUID.

// Generate TaskId type with all trait implementations
crate::define_id_type!(TaskId, "task");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_default() {
        let id1 = TaskId::default();
        let id2 = TaskId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_id_from_uuid() {
        let uuid = uuid::Uuid::new_v4();
        let id = TaskId::from(uuid);
        assert_eq!(id.as_ref(), &uuid);
    }

    #[test]
    fn test_task_id_parse() {
        let uuid = uuid::Uuid::new_v4();
        let id = TaskId::parse(uuid.to_string()).expect("Failed to parse UUID");
        assert_eq!(id.as_ref(), &uuid);
        assert!(TaskId::parse("invalid").is_err());
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::default();
        let display = format!("task:{}", &id.as_ref());
        assert_eq!(display, id.to_string());
    }
}

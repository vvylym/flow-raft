//! Benchmark utilities
//!
//! Helper functions for benchmark setup and workflow generation.

use flow_raft::api::graph::GraphBuilder;
use flow_raft::api::graph::converter::graph_to_workflow;
use flow_raft::core::{RetryConfig, WorkflowId};

/// Creates a linear workflow with N tasks
pub fn create_linear_workflow(n: usize) -> flow_raft::core::Workflow<flow_raft::core::WorkflowDraft> {
    let mut builder = GraphBuilder::new("linear_workflow");
    
    for i in 0..n {
        let task_name = format!("task{}", i);
        builder.add_node(
            &task_name,
            &format!("handler{}", i),
            vec![],
            vec![],
            None,
        );
        
        if i > 0 {
            builder.add_simple_edge(&format!("task{}", i - 1), &task_name);
        }
    }
    
    builder.set_root("task0");
    let graph = builder.build().unwrap();
    
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({})).unwrap()
}

/// Creates a parallel workflow with N branches
pub fn create_parallel_workflow(n: usize) -> flow_raft::core::Workflow<flow_raft::core::WorkflowDraft> {
    let mut builder = GraphBuilder::new("parallel_workflow");
    
    builder.add_node("start", "start_handler", vec![], vec![], None);
    builder.add_node("merge", "merge_handler", vec![], vec![], None);
    
    for i in 0..n {
        let task_name = format!("branch{}", i);
        builder.add_node(
            &task_name,
            &format!("handler{}", i),
            vec![],
            vec![],
            None,
        );
        builder.add_simple_edge("start", &task_name);
        builder.add_simple_edge(&task_name, "merge");
    }
    
    builder.set_root("start");
    let graph = builder.build().unwrap();
    
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({})).unwrap()
}

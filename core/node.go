package core

import "context"

// NodeStatus represents the operational state of a node in the cluster.
// The orchestrator uses this to determine whether runners can be scheduled on the node.
type NodeStatus string

const (
	// NodeStatusDisabled indicates the node is manually or automatically marked as disabled
	// (e.g., during maintenance). No runners will be scheduled on this node.
	NodeStatusDisabled = NodeStatus("disabled")

	// NodeStatusHealthy indicates the node is operational and available for scheduling runners.
	NodeStatusHealthy = NodeStatus("healthy")

	// NodeStatusUnhealthy indicates the node is not reporting health correctly, is unreachable,
	// or has failed health checks. Runners will not be scheduled on this node.
	NodeStatusUnhealthy = NodeStatus("unhealthy")

	// NodeStatusUnknown indicates the orchestrator hasn't received a heartbeat from the node
	// recently and its current state is unknown. Runners will not be scheduled on this node.
	NodeStatusUnknown = NodeStatus("unknown")
)

// Node represents a worker machine that runs an Agent which starts
// GitHub Actions runners via Firecracker VMs. Each node has resource
// limits (RAM, CPU, storage) and a CPU architecture that determine how
// many runners can be scheduled on it.
//
// The Orchestrator uses nodes to schedule and distribute runner workloads
// based on their available resources and architecture compatibility.
type Node struct {
	ID      int64      `json:"id"`
	Name    string     `json:"name"`
	Arch    string     `json:"arch"`
	Address string     `json:"address"`
	Status  NodeStatus `json:"status"`
	RAM     int64      `json:"ram"`
	CPU     int        `json:"cpu"`
	Storage int64      `json:"storage"`
	Created int64      `json:"created"`
	Updated int64      `json:"updated"`
	Token   string     `json:"-"`
}

// NodeClient defines operations for communicating with a node agent via RPC.
type NodeClient interface {
	// Ping pings a node to confirm connectivity.
	Ping(ctx context.Context, node *Node) error
}

// NodeStore defines operations for working with nodes on a datastore.
type NodeStore interface {
	// Create persists a new node to the datastore.
	Create(ctx context.Context, node *Node) error

	// Delete deletes a node from the datastore.
	Delete(ctx context.Context, node *Node) error

	// Find returns a node from the datastore by its ID.
	Find(ctx context.Context, id int64) (*Node, error)

	// FindName returns a node from the datastore by its name.
	FindName(ctx context.Context, name string) (*Node, error)

	// List returns a list of nodes from the datastore.
	List(ctx context.Context) ([]*Node, error)

	// ListStatus returns a list of nodes from the datastore by status.
	ListStatus(ctx context.Context, status NodeStatus) ([]*Node, error)

	// Update persists an updated node to the datastore.
	Update(ctx context.Context, node *Node) error
}

package core

import "net/http"

// Action represents the action of a hook.
type Action string

const (
	ActionCompleted  Action = "completed"
	ActionInProgress Action = "in_progress"
	ActionQueued     Action = "queued"
	ActionWaiting    Action = "waiting"
)

// Hook represents the payload of a hook.
type Hook struct {
	Action          Action   `json:"action"`
	Conclusion      string   `json:"conclusion"`
	InstallationID  int64    `json:"installation_id"`
	JobID           int64    `json:"job_id"`
	Labels          []string `json:"labels"`
	Owner           string   `json:"owner"`
	RunnerGroupID   int64    `json:"runner_group_id,omitempty"`
	RunnerGroupName string   `json:"runner_group_name,omitempty"`
	RunnerID        int64    `json:"runner_id,omitempty"`
	RunnerName      string   `json:"runner_name,omitempty"`
	URL             string   `json:"url"`
	WorkflowName    string   `json:"workflow_name"`
}

// HookParser parses a hook.
type HookParser interface {
	Parse(req *http.Request) (*Hook, error)
}

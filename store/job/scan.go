package job

import (
	"database/sql"
	"encoding/json"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// helper function converts the Job structure to a set
// of named query parameters.
func toParams(j *core.Job) map[string]interface{} {
	// Serialize labels as JSON
	labels, err := json.Marshal(j.Labels)
	if err != nil {
		// If marshaling fails, use empty array
		labels = []byte("[]")
	}

	return map[string]interface{}{
		"job_id":              j.ID,
		"job_run_id":          j.RunID,
		"job_installation_id": j.InstallationID,
		"job_owner":           j.Owner,
		"job_repo":            j.Repo,
		"job_workflow":        j.Workflow,
		"job_name":            j.Name,
		"job_branch":          j.Branch,
		"job_sha":             j.SHA,
		"job_status":          j.Status,
		"job_conclusion":      j.Conclusion,
		"job_labels":          string(labels),
		"job_runner_id":       j.RunnerID,
		"job_runner_name":     j.RunnerName,
		"job_url":             j.URL,
		"job_os":              j.OS,
		"job_arch":            j.Arch,
		"job_memory":          j.Memory,
		"job_vcpu":            j.VCPU,
		"job_accepted":        j.Accepted,
		"job_queued":          j.Queued,
		"job_started":         j.Started,
		"job_completed":       j.Completed,
		"job_created":         j.Created,
		"job_updated":         j.Updated,
		"job_version":         j.Version,
	}
}

// helper function scans the sql.Row and copies the column
// values to the destination object.
func scanRow(scanner db.Scanner, dest *core.Job) error {
	var labels string

	err := scanner.Scan(
		&dest.ID,
		&dest.RunID,
		&dest.InstallationID,
		&dest.Owner,
		&dest.Repo,
		&dest.Workflow,
		&dest.Name,
		&dest.Branch,
		&dest.SHA,
		&dest.Status,
		&dest.Conclusion,
		&labels,
		&dest.RunnerID,
		&dest.RunnerName,
		&dest.URL,
		&dest.OS,
		&dest.Arch,
		&dest.Memory,
		&dest.VCPU,
		&dest.Accepted,
		&dest.Queued,
		&dest.Started,
		&dest.Completed,
		&dest.Created,
		&dest.Updated,
		&dest.Version,
	)
	if err != nil {
		return err
	}

	// Deserialize labels from JSON
	if labels != "" && labels != "null" {
		if err := json.Unmarshal([]byte(labels), &dest.Labels); err != nil {
			return err
		}
	}

	return nil
}

// helper function scans the sql.Rows and copies the column
// values to the destination object.
func scanRows(rows *sql.Rows) ([]*core.Job, error) {
	jobs := []*core.Job{}
	for rows.Next() {
		job := new(core.Job)
		err := scanRow(rows, job)
		if err != nil {
			return nil, err
		}
		jobs = append(jobs, job)
	}

	return jobs, nil
}

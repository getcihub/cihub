package machine

import (
	"database/sql"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// toParams converts the Machine structure to a set of named query parameters.
func toParams(m *core.Machine) map[string]interface{} {
	return map[string]interface{}{
		"machine_name":      m.Name,
		"machine_owner":     m.Owner,
		"machine_arch":      m.Arch,
		"machine_cpu":       m.CPU,
		"machine_ram":       m.RAM,
		"machine_status":    m.Status,
		"machine_created":   m.Created,
		"machine_last_seen": m.LastSeen,
		"machine_updated":   m.Updated,
		"machine_token":     m.Token,
	}
}

// scanRow scans a sql.Row and copies the column values to the destination object.
func scanRow(scanner db.Scanner, dest *core.Machine) error {
	return scanner.Scan(
		&dest.Name,
		&dest.Owner,
		&dest.Arch,
		&dest.CPU,
		&dest.RAM,
		&dest.Status,
		&dest.Created,
		&dest.LastSeen,
		&dest.Updated,
		&dest.Token,
	)
}

// nullRunner is a wrapper for sql.NullString fields to support nullable runner data
// from LEFT JOIN queries where a runner may not exist for a machine.
type nullRunner struct {
	Name           sql.NullString
	ID             sql.NullInt64
	InstallationID sql.NullInt64
	Owner          sql.NullString
	Status         sql.NullString
	Machine        sql.NullString
	Arch           sql.NullString
	CPU            sql.NullInt64
	RAM            sql.NullInt64
	Image          sql.NullString
	GroupID        sql.NullInt64
	Labels         sql.NullString
	Cancelled      sql.NullInt64
	Created        sql.NullInt64
	Accepted       sql.NullInt64
	Started        sql.NullInt64
	Stopped        sql.NullInt64
	Updated        sql.NullInt64
	Token          sql.NullString
}

// value converts a nullRunner to a core.Runner pointer, returning nil if the runner
// has no name (indicating a NULL row from a LEFT JOIN).
func (n *nullRunner) value() *core.Runner {
	if !n.Name.Valid {
		return nil
	}
	var labels []string
	if n.Labels.Valid && n.Labels.String != "" {
		labels = parseLabels(n.Labels.String)
	}
	return &core.Runner{
		Name:           n.Name.String,
		ID:             n.ID.Int64,
		InstallationID: n.InstallationID.Int64,
		Owner:          n.Owner.String,
		Status:         n.Status.String,
		Machine:        n.Machine.String,
		Arch:           n.Arch.String,
		CPU:            n.CPU.Int64,
		RAM:            n.RAM.Int64,
		GroupID:        n.GroupID.Int64,
		Labels:         labels,
		Cancelled:      n.Cancelled.Int64,
		Created:        n.Created.Int64,
		Accepted:       n.Accepted.Int64,
		Started:        n.Started.Int64,
		Stopped:        n.Stopped.Int64,
		Updated:        n.Updated.Int64,
		Token:          n.Token.String,
	}
}

// parseLabels parses a comma-separated string of labels into a slice.
func parseLabels(labels string) []string {
	if labels == "" {
		return nil
	}
	// Use the same splitting logic as the runner store
	return strings.Split(labels, ",")
}

// scanRowWithRunner scans a joined machine-runner row into both a Machine and nullRunner.
func scanRowWithRunner(scanner db.Scanner, dest *core.Machine, runner *nullRunner) error {
	return scanner.Scan(
		// Machine columns
		&dest.Name,
		&dest.Owner,
		&dest.Arch,
		&dest.CPU,
		&dest.RAM,
		&dest.Status,
		&dest.Created,
		&dest.LastSeen,
		&dest.Updated,
		&dest.Token,
		// Runner columns
		&runner.Name,
		&runner.ID,
		&runner.InstallationID,
		&runner.Owner,
		&runner.Status,
		&runner.Machine,
		&runner.Arch,
		&runner.CPU,
		&runner.RAM,
		&runner.Image,
		&runner.GroupID,
		&runner.Labels,
		&runner.Cancelled,
		&runner.Created,
		&runner.Accepted,
		&runner.Started,
		&runner.Stopped,
		&runner.Updated,
		&runner.Token,
	)
}

// scanRowsWithRunners scans sql.Rows from a machines LEFT JOIN runners query,
// grouping runners under their parent machines.
func scanRowsWithRunners(rows *sql.Rows) ([]*core.Machine, error) {
	defer func() {
		if err := rows.Close(); err != nil {
			logrus.WithError(err).
				Warnln("store: cannot close machine rows")
		}
	}()

	machines := []*core.Machine{}
	var curr *core.Machine
	for rows.Next() {
		machine := new(core.Machine)
		runner := new(nullRunner)
		err := scanRowWithRunner(rows, machine, runner)
		if err != nil {
			return nil, err
		}
		// If we've moved to a new machine, add the current one and start a new one
		if curr == nil || curr.Name != machine.Name {
			curr = machine
			machines = append(machines, machine)
		}
		// If there's a runner in this row, add it to the current machine
		if r := runner.value(); r != nil {
			curr.Runners = append(curr.Runners, r)
		}
	}

	return machines, nil
}

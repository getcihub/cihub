package rpc

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/store/shared/db"
)

// HandleAccept returns an http.HandlerFunc that processes an
// http.Request to accept ownership of the runner.
//
// POST /rpc/v1/accept
func HandleAccept(runners core.RunnerStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		log := logger.FromRequest(r)
		machine, _ := MachineFrom(r.Context())

		in := new(core.Runner)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Debugln("manager: cannot decode accept request payload")
			return
		}

		runner, err := runners.Find(r.Context(), in.Name)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Warnf("manager: cannot find runner %s", in.Name)
			return
		}

		if runner.Machine != "" {
			writeError(w, db.ErrOptimisticLock)
			log.WithField("machine", runner.Machine).
				Debugln("manager: runner already assigned. abort.")
			return
		}

		now := time.Now()

		runner.Accepted = now.Unix()
		runner.Machine = machine.Name
		runner.Updated = now.Unix()

		err = runners.Update(r.Context(), runner)
		if err == db.ErrOptimisticLock {
			log = log.WithError(err)
			log.Debugln("manager: runner processed by another agent")
		} else if err != nil {
			log = log.WithError(err)
			log.Debugln("manager: cannot update runner")
		} else {
			log.Debugln("manager: runner accepted")
		}

		if err != nil {
			writeError(w, err)
		} else {
			writeOK(w)
		}
	}
}

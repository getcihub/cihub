package rpc

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleStarted returns an http.HandlerFunc that processes an
// http.Request to indicate a runner as started.
//
// POST /rpc/v1/started
func HandleStarted(runners core.RunnerStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		log := logger.FromRequest(r)

		in := new(core.Runner)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Debugln("manager: cannot decode accept request payload")
			return
		}

		log = log.WithField("runner_name", in.Name)

		runner, err := runners.Find(r.Context(), in.Name)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Warnf("manager: cannot find runner %s", in.Name)
			return
		}

		runner.Status = core.RunnerStatusIdle
		runner.Updated = time.Now().Unix()

		err = runners.Update(r.Context(), runner)
		if err != nil {
			log = log.WithError(err)
			log.Debugln("manager: cannot update runner")
		} else {
			log.Debugln("manager: runner started")
		}

		if err != nil {
			writeError(w, err)
		} else {
			writeOK(w)
		}
	}
}

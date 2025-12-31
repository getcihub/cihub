package rpc

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleRegister returns an http.HandlerFunc that processes an
// http.Request to register GitHub runner and get details
//
// POST /rpc/v1/register
func HandleRegister(runners core.RunnerStore, runnerz core.RunnerService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		in := new(core.Runner)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			writeError(w, err)
			logger.FromRequest(r).
				WithError(err).
				Debugln("rpc: cannot decode registering request payload")
			return
		}

		runner, err := runners.Find(r.Context(), in.Name)
		if err != nil {
			writeError(w, err)
			logger.FromRequest(r).
				WithError(err).
				Debugln("rpc: cannot find job")
			return
		}

		// Runner already registered
		if runner.ID != 0 && runner.Token != "" {
			writeJSON(w, &core.RunnerWithToken{
				Runner: runner,
				Token:  runner.Token,
			})
		}

		opts := core.RegisterRunnerOpts{
			InstallationID: runner.InstallationID,
			Name:           runner.Name,
			Owner:          runner.Owner,
			Labels:         runner.Labels,
			GroupID:        1,
		}

		jit, err := runnerz.Register(r.Context(), opts)
		if err != nil {
			writeError(w, err)
			logger.FromRequest(r).
				WithError(err).
				Warnln("rpc: cannot register runner")
			return
		}

		runner.ID = jit.ID
		runner.Status = core.RunnerStatusRegistered
		runner.Token = jit.Token
		runner.Updated = time.Now().Unix()

		err = runners.Update(r.Context(), runner)
		if err != nil {
			writeError(w, err)
			logger.FromRequest(r).
				WithError(err).
				Warnln("rpc: cannot update runner")
			return
		}

		writeJSON(w, &core.RunnerWithToken{
			Runner: runner,
			Token:  runner.Token,
		})
	}
}

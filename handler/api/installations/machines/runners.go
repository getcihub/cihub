package machines

import (
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleRunners returns an http.HandlerFunc that processes http
// requests to list all runners of a machine.
func HandleRunners(machines core.MachineStore, runners core.RunnerStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			owner = chi.URLParam(r, "owner")
			name  = chi.URLParam(r, "name")
		)

		machine, err := machines.Find(r.Context(), owner, name)
		if err != nil {
			render.NotFound(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("name", name).
				Debugln("api: cannot find machine")
			return
		}

		results, err := runners.ListMachine(r.Context(), machine)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("name", name).
				Debugln("api: cannot list machine runners")
			return
		}

		render.OK(w, render.ReasonListed, results)
	}
}

package machines

import (
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleFind returns an http.HandlerFunc that writes json-encoded
// machine details to the response body.
func HandleFind(machines core.MachineStore) http.HandlerFunc {
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
				Debugln("api: machine not found")
			return
		}

		render.OK(w, render.ReasonResolved, machine)
	}
}

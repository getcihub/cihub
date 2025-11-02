package jobs

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleList returns an http.HandlerFunc that writes a json-encoded
// paginated list of jobs from an owner to the response body.
func HandleList(jobs core.JobStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		status := r.FormValue("status")
		if status == "" {
			status = core.JobStatusQueued
		}

		results, err := jobs.ListStatus(r.Context(), status)
		if err != nil {
			render.InternalError(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("status", status).
				Warnln("api: cannot list incomplete jobs")
			return
		}

		render.OK(w, render.ReasonListed, results)
	}
}

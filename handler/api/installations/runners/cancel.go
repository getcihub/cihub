package runners

import (
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
	"github.com/go-chi/chi/v5"
)

// HandleCancel returns an http.HandlerFunc that processes http
// requests to cancel a pending or running runner.
func HandleCancel(runners core.RunnerStore, runnerz core.RunnerService, scheduler core.Scheduler) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			owner = chi.URLParam(r, "owner")
			name  = chi.URLParam(r, "name")
		)

		runner, err := runners.Find(r.Context(), name)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("name", name).
				Debugln("api: cannot find runner")
			render.NotFound(w)
			return
		}

		if runner.Status != core.RunnerStatusCompleted {
			runner.Status = core.RunnerStatusCompleted
			runner.Cancelled = time.Now().Unix()
			if runner.Started == 0 {
				runner.Started = time.Now().Unix()
			}

			err = runners.Update(r.Context(), runner)
			if err != nil {
				logger.FromRequest(r).
					WithError(err).
					WithField("owner", owner).
					WithField("name", name).
					Warnln("api: cannot update runner status to be cancelled")
				render.NotFound(w)
				return
			}

			err = scheduler.Cancel(r.Context(), runner.Name)
			if err != nil {
				logger.FromRequest(r).
					WithError(err).
					WithField("owner", owner).
					WithField("name", name).
					Warnln("api: cannot signal cancelled runner is complete")
			}

			err = runnerz.Delete(r.Context(), runner)
			if err != nil {
				logger.FromRequest(r).
					WithError(err).
					WithField("owner", owner).
					WithField("name", name).
					Warnln("api: cannot delete runner from github")
			}
		}

		render.NoContent(w)
	}
}

package web

import (
	"context"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
	"github.com/sirupsen/logrus"
)

// HandleHook returns an http.HandlerFunc that handles
// webhooks triggered by GitHub.
func HandleHook(
	runners core.RunnerStore,
	triggerer core.Triggerer,
	parser core.HookParser,
) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		hook, err := parser.Parse(r)
		if err != nil {
			render.BadRequestWithReason(w, err.Error())
			logger.FromRequest(r).
				WithError(err).
				Warnln("web: cannot parse webhook")
			return
		}

		if hook == nil {
			render.Accepted(w, nil)
			logger.FromRequest(r).
				Debugln("web: webhook ignored")
			return
		}

		log := logger.FromRequest(r).
			WithFields(logrus.Fields{
				"action":          hook.Action,
				"installation_id": hook.InstallationID,
				"job_id":          hook.JobID,
				"owner":           hook.Owner,
			})

		ctx, cancel := context.WithTimeout(context.Background(), time.Minute*5)
		ctx = logger.WithContext(ctx, log)
		defer cancel()

		if hook.Action == core.ActionCompleted {
			log.WithField("conclusion", hook.Conclusion).Debugln("hook: runner has completed")
			runners.UpdateStatus(ctx, hook.RunnerID, core.RunnerStatusCompleted)
			render.Accepted(w, nil)
			return
		}

		if hook.Action == core.ActionInProgress {
			log.Debugln("hook: runner has started")
			runners.UpdateStatus(ctx, hook.RunnerID, core.RunnerStatusBusy)
			render.Accepted(w, nil)
			return
		}

		runner, err := triggerer.Trigger(ctx, hook)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			return
		}

		render.Accepted(w, runner)
	}
}

package acl

import (
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// CheckAccess returns an http.Handler middleware that authorizes only
// authenticated users with the required member or admin access
// membership to the requested installation resource.
func CheckAccess(service core.InstallationService, admin bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			owner := chi.URLParam(r, "owner")
			log := logger.FromRequest(r)
			ctx := r.Context()

			user, ok := request.UserFrom(ctx)
			if !ok {
				render.Unauthorized(w)
				log.Debugln("api: authentication required for access")
				return
			}
			log = log.WithField("user.admin", user.Admin)

			// if the user is an administrator they are always
			// granted access to the organization data.
			if user.Admin {
				next.ServeHTTP(w, r)
				return
			}

			if user.Login == owner {
				next.ServeHTTP(w, r)
				return
			}

			isMember, isAdmin, err := service.Membership(ctx, user, owner)
			if err != nil {
				render.Forbidden(w)
				log.Debugln("api: installation membership not found")
				return
			}

			log = log.
				WithField("installation.member", isMember).
				WithField("installation.admin", isAdmin)

			if !isMember {
				render.Forbidden(w)
				log.Debugln("api: installation membership is required")
				return
			}

			if isAdmin == false && admin == true {
				render.Forbidden(w)
				log.Debugln("api: installation administrator is required")
				return
			}

			log.Debugln("api: installation membership verified")
			next.ServeHTTP(w, r)
		})
	}
}

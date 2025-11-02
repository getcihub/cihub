package acl

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// CheckMember returns an http.Handler middleware that authorizes only
// authenticated users with membership access to proceed to the next
// handler in the chain.
func CheckMember() func(http.Handler) http.Handler {
	return CheckAccess(true, false)
}

// CheckAdmin returns an http.Handler middleware that authorizes only
// authenticated users with admin membership access to proceed to the next
// handler in the chain.
func CheckAdmin() func(http.Handler) http.Handler {
	return CheckAccess(true, true)
}

// CheckAccess returns an http.Handler middleware that authorizes only
// authenticated users with the required member or admin access
// membership to the requested organization resource.
func CheckAccess(member, admin bool) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			log := logger.FromRequest(r)
			ctx := r.Context()
			user, _ := request.UserFrom(ctx)

			installation, noInstallation := request.InstallationFrom(ctx)
			if !noInstallation {
				// this should never happen. the installation
				// should always be injected into the context
				// by an upstream handler in the chain.
				log.Errorln("api: null installation in context")
				render.NotFound(w)
				return
			}

			membership, ok := request.MembershipFrom(ctx)
			switch {
			case !ok && installation.Login == user.Login:
				log.Debugln("api: user account access granted")
				next.ServeHTTP(w, r)
				return
			case ok && member:
				log.Debugln("api: org member access granted")
				next.ServeHTTP(w, r)
				return
			case ok && admin && membership.Role == core.MembershipRoleAdmin:
				log.Debugln("api: org admin access granted")
				next.ServeHTTP(w, r)
				return
			default:
				render.Unauthorized(w)
				log.Debugln("api: authentication required")
				return
			}
		})
	}
}

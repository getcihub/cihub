package user

import (
	"github.com/google/go-github/v75/github"

	"github.com/getcihub/cihub/core"
)

func convertUser(from *github.User) *core.User {
	return &core.User{
		Avatar:  from.GetAvatarURL(),
		Email:   from.GetEmail(),
		Login:   from.GetLogin(),
		Created: from.GetCreatedAt().Unix(),
		Updated: from.GetUpdatedAt().Unix(),
	}
}

func convertEmailList(from []*github.UserEmail) []*core.Email {
	to := []*core.Email{}
	for _, v := range from {
		to = append(to, convertEmail(v))
	}
	return to
}

func convertEmail(from *github.UserEmail) *core.Email {
	return &core.Email{
		Email:    from.GetEmail(),
		Primary:  from.GetPrimary(),
		Verified: from.GetVerified(),
	}
}

func returnPrimaryEmail(from []*core.Email) *core.Email {
	for _, v := range from {
		if v.Primary {
			return v
		}
	}
	return nil
}

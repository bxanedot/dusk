# Dusk Supabase email branding

Subject: `Confirm your Dusk account`

The confirmation template in this folder is the Dusk-branded Supabase Auth email. Hosted Supabase projects store email-template configuration outside Postgres, so it cannot be applied through SQL migrations. The desktop registration flow now verifies accounts server-side and does not depend on the old localhost confirmation redirect. If email confirmation is enabled again, paste `confirmation.html` into Authentication -> Emails -> Confirm signup.

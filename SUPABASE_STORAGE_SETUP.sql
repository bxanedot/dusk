-- Dusk account + private cloud-save setup.
-- The desktop app uses only the project URL and publishable key.
-- Never embed the service_role key in Dusk.

create schema if not exists private;

create table if not exists public.dusk_accounts (
  user_id uuid primary key references auth.users(id) on delete cascade,
  username text not null,
  username_normalized text not null unique,
  email text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint dusk_accounts_username_format check (username ~ '^[A-Za-z0-9_.-]{3,24}$'),
  constraint dusk_accounts_username_normalized check (username_normalized = lower(username))
);
alter table public.dusk_accounts enable row level security;

drop policy if exists "dusk_accounts_select_own" on public.dusk_accounts;
create policy "dusk_accounts_select_own" on public.dusk_accounts
for select to authenticated using ((select auth.uid()) = user_id);
grant select on public.dusk_accounts to authenticated;
revoke all on public.dusk_accounts from anon;

create table if not exists public.dusk_account_state (
  user_id uuid primary key references auth.users(id) on delete cascade,
  state jsonb not null default '{}'::jsonb,
  state_version integer not null default 1,
  updated_at timestamptz not null default now()
);
alter table public.dusk_account_state enable row level security;

drop policy if exists "dusk_state_select_own" on public.dusk_account_state;
create policy "dusk_state_select_own" on public.dusk_account_state
for select to authenticated using ((select auth.uid()) = user_id);
drop policy if exists "dusk_state_insert_own" on public.dusk_account_state;
create policy "dusk_state_insert_own" on public.dusk_account_state
for insert to authenticated with check ((select auth.uid()) = user_id);
drop policy if exists "dusk_state_update_own" on public.dusk_account_state;
create policy "dusk_state_update_own" on public.dusk_account_state
for update to authenticated
using ((select auth.uid()) = user_id)
with check ((select auth.uid()) = user_id);
drop policy if exists "dusk_state_delete_own" on public.dusk_account_state;
create policy "dusk_state_delete_own" on public.dusk_account_state
for delete to authenticated using ((select auth.uid()) = user_id);
grant select, insert, update, delete on public.dusk_account_state to authenticated;
revoke all on public.dusk_account_state from anon;

create or replace function private.handle_dusk_user_created()
returns trigger language plpgsql security definer set search_path = ''
as $$
declare requested_username text;
begin
  requested_username := trim(coalesce(new.raw_user_meta_data ->> 'username', ''));
  if requested_username = '' then raise exception 'A Dusk username is required.'; end if;
  insert into public.dusk_accounts (user_id, username, username_normalized, email)
  values (new.id, requested_username, lower(requested_username), coalesce(new.email, ''));
  insert into public.dusk_account_state (user_id) values (new.id)
  on conflict (user_id) do nothing;
  return new;
end;
$$;
revoke all on function private.handle_dusk_user_created() from public, anon, authenticated;

drop trigger if exists on_dusk_auth_user_created on auth.users;
create trigger on_dusk_auth_user_created
after insert on auth.users for each row execute function private.handle_dusk_user_created();

insert into storage.buckets (id, name, public)
values ('dusk-savefiles', 'dusk-savefiles', false)
on conflict (id) do update set public = false;

drop policy if exists "dusk_savefiles_insert_own" on storage.objects;
create policy "dusk_savefiles_insert_own" on storage.objects
for insert to authenticated
with check (bucket_id='dusk-savefiles' and (storage.foldername(name))[1]=(select auth.uid()::text));

drop policy if exists "dusk_savefiles_select_own" on storage.objects;
create policy "dusk_savefiles_select_own" on storage.objects
for select to authenticated
using (bucket_id='dusk-savefiles' and (storage.foldername(name))[1]=(select auth.uid()::text));

drop policy if exists "dusk_savefiles_update_own" on storage.objects;
create policy "dusk_savefiles_update_own" on storage.objects
for update to authenticated
using (bucket_id='dusk-savefiles' and (storage.foldername(name))[1]=(select auth.uid()::text))
with check (bucket_id='dusk-savefiles' and (storage.foldername(name))[1]=(select auth.uid()::text));

drop policy if exists "dusk_savefiles_delete_own" on storage.objects;
create policy "dusk_savefiles_delete_own" on storage.objects
for delete to authenticated
using (bucket_id='dusk-savefiles' and (storage.foldername(name))[1]=(select auth.uid()::text));

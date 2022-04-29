use std::result::Result;
use super::super::private::git;
use super::super::super::ps;
use super::super::private::utils;
use super::super::private::string_file_io::{write_str_to_file, WriteStrToFileError, read_str_from_file, ReadStringFromFileError};
use super::super::private::paths;
use super::super::private::hooks;

#[derive(Debug)]
pub enum IsolateError {
  OpenGitRepositoryFailed(git::CreateCwdRepositoryError),
  GetPatchStackFailed(ps::PatchStackError),
  GetPatchListFailed(ps::GetPatchListError),
  PatchIndexNotFound,
  PatchStackBaseNotFound,
  CreateBranchFailed,
  BranchNameNotUtf8,
  CherryPickFailed,
  FailedToCheckout(utils::ExecuteError),
  GetCurrentBranchFailed,
  GetIsolateLastBranchPathFailed(paths::PathsError),
  StoreLastBranchFailed(WriteStrToFileError),
  ReadLastBranchFailed(ReadStringFromFileError),
  OpenGitConfigFailed(git2::Error),
  GetRepoRootPathFailed(paths::PathsError),
  PathNotUtf8,
  HookNotFound(hooks::FindHookError),
  HookExecutionFailed(utils::ExecuteError),
  UncommittedChangesExistFailure(git::UncommittedChangesError),
  UncommittedChangesExist,
  FindIsolateBranchFailed(git2::Error),
  DeleteIsolateBranchFailed(git2::Error)
}

pub fn isolate(patch_index_optional: Option<usize>) -> Result<(), IsolateError> {
  let isolate_branch_name = "ps/tmp/isolate";
  let repo = ps::private::git::create_cwd_repo().map_err(IsolateError::OpenGitRepositoryFailed)?;
  let config = git2::Config::open_default().map_err(IsolateError::OpenGitConfigFailed)?;

  if git::uncommitted_changes_exist(&repo).map_err(IsolateError::UncommittedChangesExistFailure)? {
    return Err(IsolateError::UncommittedChangesExist);
  }

  match patch_index_optional {
    Some(patch_index) => {
      let patch_stack = ps::get_patch_stack(&repo).map_err(IsolateError::GetPatchStackFailed)?;
      let patch_stack_base_commit = patch_stack.base.peel_to_commit().map_err(|_| IsolateError::PatchStackBaseNotFound)?;
      let patches_vec = ps::get_patch_list(&repo, patch_stack).map_err(IsolateError::GetPatchListFailed)?;
      let patch_oid = patches_vec.get(patch_index).ok_or(IsolateError::PatchIndexNotFound)?.oid;

      let branch = repo.branch(isolate_branch_name, &patch_stack_base_commit, true).map_err(|_| IsolateError::CreateBranchFailed)?;

      let branch_ref_name = branch.get().name().ok_or(IsolateError::BranchNameNotUtf8)?;

      // - cherry pick the patch onto new rr branch
      git::cherry_pick_no_working_copy(&repo, &config, patch_oid, branch_ref_name).map_err(|_| IsolateError::CherryPickFailed)?;

      // get currently checked out branch name
      let checked_out_branch = git::get_current_branch_shorthand(&repo).ok_or(IsolateError::GetCurrentBranchFailed)?;
      // write currently checked out branch name to disk
      let path = paths::isolate_last_branch_path(&repo).map_err(IsolateError::GetIsolateLastBranchPathFailed)?;
      write_str_to_file(checked_out_branch.as_str(), path).map_err(IsolateError::StoreLastBranchFailed)?;

      // checkout the ps/tmp/checkout branch
      utils::execute("git", &["checkout", isolate_branch_name]).map_err(IsolateError::FailedToCheckout)?;

      let repo_root_path = paths::repo_root_path(&repo).map_err(IsolateError::GetRepoRootPathFailed)?;
      let repo_root_str = repo_root_path.to_str().ok_or(IsolateError::PathNotUtf8)?;
      match hooks::find_hook(repo_root_str, "isolate_post_checkout") {
        Ok(hook_path) => utils::execute(hook_path.to_str().ok_or(IsolateError::PathNotUtf8)?, &[]).map_err(IsolateError::HookExecutionFailed)?,
        Err(hooks::FindHookError::NotFound) => eprintln!("Warning: isolate_post_checkout hook not found. Skipping hook execution."),
        Err(hooks::FindHookError::NotExecutable(hook_path)) => eprintln!("Warning: hook {} found, but is not executable. Skipping hook execution.", hook_path.to_str().ok_or(IsolateError::PathNotUtf8)?),
        Err(e) => return Err(IsolateError::HookNotFound(e))
      }

      Ok(())
    },
    None => {
      // read last checked out branch name from disk
      let path = paths::isolate_last_branch_path(&repo).map_err(IsolateError::GetIsolateLastBranchPathFailed)?;
      let last_branch = read_str_from_file(path).map_err(IsolateError::ReadLastBranchFailed)?;

      // check it out
      utils::execute("git", &["checkout", &last_branch]).map_err(IsolateError::FailedToCheckout)?;

      let mut isolate_branch = repo.find_branch(isolate_branch_name, git2::BranchType::Local).map_err(IsolateError::FindIsolateBranchFailed)?;
      isolate_branch.delete().map_err(IsolateError::DeleteIsolateBranchFailed)?;

      Ok(())
    }
  }
}

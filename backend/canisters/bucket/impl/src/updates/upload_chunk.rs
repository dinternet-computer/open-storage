use crate::guards::caller_is_known_user;
use crate::model::files::{PutChunkArgs, PutChunkResult};
use crate::model::index_sync_state::EventToSync;
use crate::model::users::{FileStatusInternal, IndexSyncComplete};
use crate::{mutate_state, RuntimeState};
use bucket_canister::upload_chunk_v2::{Response::*, *};
use canister_api_macros::trace;
use ic_cdk_macros::update;
use types::{FileRemoved, RejectedReason, UserId};

#[update(guard = "caller_is_known_user")]
#[trace]
fn upload_chunk_v2(args: Args) -> Response {
    mutate_state(|state| upload_chunk_impl(args, state))
}

fn upload_chunk_impl(args: Args, runtime_state: &mut RuntimeState) -> Response {
    let user_id: UserId = runtime_state.env.caller();
    let now = runtime_state.env.now();
    let user = runtime_state.data.users.get_mut(&user_id).unwrap();
    let file_id = args.file_id;

    let mut index_sync_complete = IndexSyncComplete::No;
    if let Some(status) = user.file_status(&file_id) {
        match status {
            FileStatusInternal::Complete(_) | FileStatusInternal::Rejected(RejectedReason::HashMismatch) => {
                return FileAlreadyExists
            }
            FileStatusInternal::Rejected(RejectedReason::AllowanceExceeded) => return AllowanceExceeded,
            FileStatusInternal::Rejected(RejectedReason::UserNotFound) => return UserNotFound,
            FileStatusInternal::Uploading(c) => index_sync_complete = *c,
        }
    } else {
        user.set_file_status(file_id, FileStatusInternal::Uploading(IndexSyncComplete::No));
    }

    match runtime_state.data.files.put_chunk(PutChunkArgs::new(user_id, args, now)) {
        PutChunkResult::Success(r) => {
            if r.file_completed {
                user.set_file_status(file_id, FileStatusInternal::Complete(index_sync_complete));
            }
            if let Some(file_added) = r.file_added {
                runtime_state
                    .data
                    .index_sync_state
                    .enqueue(EventToSync::FileAdded(file_added));
            }
            Success
        }
        PutChunkResult::FileAlreadyExists => FileAlreadyExists,
        PutChunkResult::FileTooBig(_) => FileTooBig,
        PutChunkResult::ChunkAlreadyExists => ChunkAlreadyExists,
        PutChunkResult::ChunkIndexTooHigh => ChunkIndexTooHigh,
        PutChunkResult::ChunkSizeMismatch(_) => ChunkSizeMismatch,
        PutChunkResult::HashMismatch(hm) => {
            // When there is a hash mismatch, the file has already been removed from the list of
            // pending files, so we now need to update the status and tell the index canister to
            // remove the file reference.
            user.set_file_status(file_id, FileStatusInternal::Rejected(RejectedReason::HashMismatch));

            // We only need to remove the file reference from the index canister if this file
            // consists of multiple chunks. If the file is a single chunk then the Success case of
            // this match statement will never have been reached so the file reference will not have
            // been added to the index canister.
            if hm.chunk_count > 1 {
                runtime_state
                    .data
                    .index_sync_state
                    .enqueue(EventToSync::FileRemoved(FileRemoved {
                        file_id,
                        owner: user_id,
                        hash: hm.provided_hash,
                        blob_deleted: !runtime_state.data.files.contains_hash(&hm.provided_hash),
                    }));
            }

            HashMismatch
        }
    }
}

//! What a sign-in, a caller's check or a device key's challenge answers, each under the kind
//! the contract publishes for it.

use serde::Serialize;

use super::{Admitted, Calling, SignInCode};
use crate::device_keys::Challenged;
use crate::model::kind::{self, Kind};
use crate::model::Envelope;

/// What a sign-in, a caller's check or a device key's challenge answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// A session opened, and the moment it stops being one.
    Admission(Admitted),
    /// A code to sign in by, or the sign-in by a code still waiting.
    SignInCode(SignInCode),
    /// Who the session or key a request carried proves it to be.
    Caller(Calling),
    /// A challenge for a device key to sign, and when it stops standing.
    Challenge(Challenged),
}

impl Answer {
    /// The kind this answer is published under.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Admission(_) => kind::ADMISSION,
            Self::SignInCode(_) => kind::SIGN_IN_CODE,
            Self::Caller(_) => kind::CALLER,
            Self::Challenge(_) => kind::DEVICE_CHALLENGE,
        }
    }

    /// The shape of each answer, under its kind.
    pub fn schemas(each: &mut impl FnMut(Kind, schemars::Schema)) {
        each(kind::ADMISSION, schemars::schema_for!(Envelope<Admitted>));
        each(
            kind::SIGN_IN_CODE,
            schemars::schema_for!(Envelope<SignInCode>),
        );
        each(kind::CALLER, schemars::schema_for!(Envelope<Calling>));
        each(
            kind::DEVICE_CHALLENGE,
            schemars::schema_for!(Envelope<Challenged>),
        );
    }
}

impl Serialize for Answer {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Admission(admitted) => admitted.serialize(serializer),
            Self::SignInCode(code) => code.serialize(serializer),
            Self::Caller(calling) => calling.serialize(serializer),
            Self::Challenge(challenged) => challenged.serialize(serializer),
        }
    }
}

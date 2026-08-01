use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct ObjectLimitsSummary {
    pub(crate) out_of_range_integers: Vec<PdfObjectId>,
    pub(crate) out_of_range_reals: Vec<PdfObjectId>,
    pub(crate) overlong_strings: Vec<PdfObjectId>,
    pub(crate) overlong_names: Vec<PdfObjectId>,
    pub(crate) oversized_arrays: Vec<PdfObjectId>,
    pub(crate) oversized_dictionaries: Vec<PdfObjectId>,
    pub(crate) too_many_indirect_objects: bool,
}

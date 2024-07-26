#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppleStandardKeyBindingAction {
    // Inserting content
    InsertBacktab,
    InsertContainerBreak,
    InsertDoubleQuoteIgnoringSubstitution,
    InsertLineBreak,
    InsertNewline,
    InsertNewlineIgnoringFieldEditor,
    InsertParagraphSeparator,
    InsertSingleQuoteIgnoringSubstitution,
    InsertTab,
    InsertTabIgnoringFieldEditor,
    InsertText,

    // Deleting content
    DeleteBackward,
    DeleteBackwardByDecomposingPreviousCharacter,
    DeleteForward,
    DeleteToBeginningOfLine,
    DeleteToBeginningOfParagraph,
    DeleteToEndOfLine,
    DeleteToEndOfParagraph,
    DeleteWordBackward,
    DeleteWordForward,
    Yank,

    // Moving the insertion pointer
    MoveBackward,
    MoveDown,
    MoveForward,
    MoveLeft,
    MoveRight,
    MoveUp,

    // Modifying the selection
    MoveBackwardAndModifySelection,
    MoveDownAndModifySelection,
    MoveForwardAndModifySelection,
    MoveLeftAndModifySelection,
    MoveRightAndModifySelection,
    MoveUpAndModifySelection,

    // Scrolling content
    ScrollPageDown,
    ScrollPageUp,
    ScrollLineDown,
    ScrollLineUp,
    ScrollToBeginningOfDocument,
    ScrollToEndOfDocument,
    PageDown,
    PageUp,
    PageUpAndModifySelection,
    PageDownAndModifySelection,
    CenterSelectionInVisibleArea,

    // Transposing elements
    Transpose,
    TransposeWords,

    // Indenting content
    Indent,

    // Cancelling operations
    CancelOperation,

    // Supporting quickLook
    QuickLookPreviewItems,

    // Supporting writing direction
    MakeBaseWritingDirectionLeftToRight,
    MakeBaseWritingDirectionNatural,
    MakeBaseWritingDirectionRightToLeft,
    MakeTextWritingDirectionLeftToRight,
    MakeTextWritingDirectionNatural,
    MakeTextWritingDirectionRightToLeft,

    // Changing captilization
    CapitalizeWord,
    ChangeCaseOfLetter,
    LowercaseWord,
    UppercaseWord,

    // Moving the selection in documents
    MoveToBeginningOfDocument,
    MoveToBeginningOfDocumentAndModifySelection,
    MoveToEndOfDocument,
    MoveToEndOfDocumentAndModifySelection,

    // Moving the selection in paragraphs
    MoveParagraphBackwardAndModifySelection,
    MoveParagraphForwardAndModifySelection,
    MoveToBeginningOfParagraph,
    MoveToBeginningOfParagraphAndModifySelection,
    MoveToEndOfParagraph,
    MoveToEndOfParagraphAndModifySelection,

    // Moving the selection in lines of text
    MoveToBeginningOfLine,
    MoveToBeginningOfLineAndModifySelection,
    MoveToEndOfLine,
    MoveToEndOfLineAndModifySelection,
    MoveToLeftEndOfLine,
    MoveToLeftEndOfLineAndModifySelection,
    MoveToRightEndOfLine,
    MoveToRightEndOfLineAndModifySelection,

    // Changing the selection
    SelectAll,
    SelectLine,
    SelectParagraph,
    SelectWord,

    // Supporting marked selections
    SetMark,
    SelectToMark,
    DeleteToMark,
    SwapWithMark,

    // Supporting autocomplete
    Complete,

    // Moving selection by word boundaries
    MoveWordBackward,
    MoveWordBackwardAndModifySelection,
    MoveWordForward,
    MoveWordForwardAndModifySelection,
    MoveWordLeft,
    MoveWordLeftAndModifySelection,
    MoveWordRight,
    MoveWordRightAndModifySelection,

    // Instance methods
    ShowContextMenuForSelection,

    // Other
    Noop,
    Other(Box<str>),
}

impl From<&str> for AppleStandardKeyBindingAction {
    fn from(s: &str) -> Self {
        match s {
            // Inserting content
            "insertBacktab:" => Self::InsertBacktab,
            "insertContainerBreak:" => Self::InsertContainerBreak,
            "insertDoubleQuoteIgnoringSubstitution:" => Self::InsertDoubleQuoteIgnoringSubstitution,
            "insertLineBreak:" => Self::InsertLineBreak,
            "insertNewline:" => Self::InsertNewline,
            "insertNewlineIgnoringFieldEditor:" => Self::InsertNewlineIgnoringFieldEditor,
            "insertParagraphSeparator:" => Self::InsertParagraphSeparator,
            "insertSingleQuoteIgnoringSubstitution:" => Self::InsertSingleQuoteIgnoringSubstitution,
            "insertTab:" => Self::InsertTab,
            "insertTabIgnoringFieldEditor:" => Self::InsertTabIgnoringFieldEditor,
            "insertText:" => Self::InsertText,

            // Deleting content
            "deleteBackward:" => Self::DeleteBackward,
            "deleteBackwardByDecomposingPreviousCharacter:" => {
                Self::DeleteBackwardByDecomposingPreviousCharacter
            },
            "deleteForward:" => Self::DeleteForward,
            "deleteToBeginningOfLine:" => Self::DeleteToBeginningOfLine,
            "deleteToBeginningOfParagraph:" => Self::DeleteToBeginningOfParagraph,
            "deleteToEndOfLine:" => Self::DeleteToEndOfLine,
            "deleteToEndOfParagraph:" => Self::DeleteToEndOfParagraph,
            "deleteWordBackward:" => Self::DeleteWordBackward,
            "deleteWordForward:" => Self::DeleteWordForward,
            "yank:" => Self::Yank,

            // Moving the insertion pointer
            "moveBackward:" => Self::MoveBackward,
            "moveDown:" => Self::MoveDown,
            "moveForward:" => Self::MoveForward,
            "moveLeft:" => Self::MoveLeft,
            "moveRight:" => Self::MoveRight,
            "moveUp:" => Self::MoveUp,

            // Modifying the selection
            "moveBackwardAndModifySelection:" => Self::MoveBackwardAndModifySelection,
            "moveDownAndModifySelection:" => Self::MoveDownAndModifySelection,
            "moveForwardAndModifySelection:" => Self::MoveForwardAndModifySelection,
            "moveLeftAndModifySelection:" => Self::MoveLeftAndModifySelection,
            "moveRightAndModifySelection:" => Self::MoveRightAndModifySelection,
            "moveUpAndModifySelection:" => Self::MoveUpAndModifySelection,

            // Scrolling content
            "scrollPageDown:" => Self::ScrollPageDown,
            "scrollPageUp:" => Self::ScrollPageUp,
            "scrollLineDown:" => Self::ScrollLineDown,
            "scrollLineUp:" => Self::ScrollLineUp,
            "scrollToBeginningOfDocument:" => Self::ScrollToBeginningOfDocument,
            "scrollToEndOfDocument:" => Self::ScrollToEndOfDocument,
            "pageDown:" => Self::PageDown,
            "pageUp:" => Self::PageUp,
            "pageUpAndModifySelection:" => Self::PageUpAndModifySelection,
            "pageDownAndModifySelection:" => Self::PageDownAndModifySelection,
            "centerSelectionInVisibleArea:" => Self::CenterSelectionInVisibleArea,

            // Transposing elements
            "transpose:" => Self::Transpose,
            "transposeWords:" => Self::TransposeWords,

            // Indenting content
            "indent:" => Self::Indent,

            // Cancelling operations
            "cancelOperation:" => Self::CancelOperation,

            // Supporting quickLook
            "quickLookPreviewItems:" => Self::QuickLookPreviewItems,

            // Supporting writing direction
            "makeBaseWritingDirectionLeftToRight:" => Self::MakeBaseWritingDirectionLeftToRight,
            "makeBaseWritingDirectionNatural:" => Self::MakeBaseWritingDirectionNatural,
            "makeBaseWritingDirectionRightToLeft:" => Self::MakeBaseWritingDirectionRightToLeft,
            "makeTextWritingDirectionLeftToRight:" => Self::MakeTextWritingDirectionLeftToRight,
            "makeTextWritingDirectionNatural:" => Self::MakeTextWritingDirectionNatural,
            "makeTextWritingDirectionRightToLeft:" => Self::MakeTextWritingDirectionRightToLeft,

            // Changing captilization
            "capitalizeWord:" => Self::CapitalizeWord,
            "changeCaseOfLetter:" => Self::ChangeCaseOfLetter,
            "lowercaseWord:" => Self::LowercaseWord,
            "uppercaseWord:" => Self::UppercaseWord,

            // Moving the selection in documents
            "moveToBeginningOfDocument:" => Self::MoveToBeginningOfDocument,
            "moveToBeginningOfDocumentAndModifySelection:" => {
                Self::MoveToBeginningOfDocumentAndModifySelection
            },
            "moveToEndOfDocument:" => Self::MoveToEndOfDocument,
            "moveToEndOfDocumentAndModifySelection:" => Self::MoveToEndOfDocumentAndModifySelection,

            // Moving the selection in paragraphs
            "moveParagraphBackwardAndModifySelection:" => {
                Self::MoveParagraphBackwardAndModifySelection
            },
            "moveParagraphForwardAndModifySelection:" => {
                Self::MoveParagraphForwardAndModifySelection
            },
            "moveToBeginningOfParagraph:" => Self::MoveToBeginningOfParagraph,
            "moveToBeginningOfParagraphAndModifySelection:" => {
                Self::MoveToBeginningOfParagraphAndModifySelection
            },
            "moveToEndOfParagraph:" => Self::MoveToEndOfParagraph,
            "moveToEndOfParagraphAndModifySelection:" => {
                Self::MoveToEndOfParagraphAndModifySelection
            },

            // Moving the selection in lines of text
            "moveToBeginningOfLine:" => Self::MoveToBeginningOfLine,
            "moveToBeginningOfLineAndModifySelection:" => {
                Self::MoveToBeginningOfLineAndModifySelection
            },
            "moveToEndOfLine:" => Self::MoveToEndOfLine,
            "moveToEndOfLineAndModifySelection:" => Self::MoveToEndOfLineAndModifySelection,
            "moveToLeftEndOfLine:" => Self::MoveToLeftEndOfLine,
            "moveToLeftEndOfLineAndModifySelection:" => Self::MoveToLeftEndOfLineAndModifySelection,
            "moveToRightEndOfLine:" => Self::MoveToRightEndOfLine,
            "moveToRightEndOfLineAndModifySelection:" => {
                Self::MoveToRightEndOfLineAndModifySelection
            },

            // Changing the selection
            "selectAll:" => Self::SelectAll,
            "selectLine:" => Self::SelectLine,
            "selectParagraph:" => Self::SelectParagraph,
            "selectWord:" => Self::SelectWord,

            // Supporting marked selections
            "setMark:" => Self::SetMark,
            "selectToMark:" => Self::SelectToMark,
            "deleteToMark:" => Self::DeleteToMark,
            "swapWithMark:" => Self::SwapWithMark,

            // Supporting autocomplete
            "complete:" => Self::Complete,

            // Moving selection by word boundaries
            "moveWordBackward:" => Self::MoveWordBackward,
            "moveWordBackwardAndModifySelection:" => Self::MoveWordBackwardAndModifySelection,
            "moveWordForward:" => Self::MoveWordForward,
            "moveWordForwardAndModifySelection:" => Self::MoveWordForwardAndModifySelection,
            "moveWordLeft:" => Self::MoveWordLeft,
            "moveWordLeftAndModifySelection:" => Self::MoveWordLeftAndModifySelection,
            "moveWordRight:" => Self::MoveWordRight,
            "moveWordRightAndModifySelection:" => Self::MoveWordRightAndModifySelection,

            // Instance methods
            "showContextMenuForSelection:" => Self::ShowContextMenuForSelection,

            // Other
            "noop:" => Self::Noop,
            _ => Self::Other(s.to_string().into_boxed_str()),
        }
    }
}

impl AppleStandardKeyBindingAction {
    pub fn as_str(&self) -> &str {
        match self {
            // Inserting content
            Self::InsertBacktab => "insertBacktab:",
            Self::InsertContainerBreak => "insertContainerBreak:",
            Self::InsertDoubleQuoteIgnoringSubstitution => "insertDoubleQuoteIgnoringSubstitution:",
            Self::InsertLineBreak => "insertLineBreak:",
            Self::InsertNewline => "insertNewline:",
            Self::InsertNewlineIgnoringFieldEditor => "insertNewlineIgnoringFieldEditor:",
            Self::InsertParagraphSeparator => "insertParagraphSeparator:",
            Self::InsertSingleQuoteIgnoringSubstitution => "insertSingleQuoteIgnoringSubstitution:",
            Self::InsertTab => "insertTab:",
            Self::InsertTabIgnoringFieldEditor => "insertTabIgnoringFieldEditor:",
            Self::InsertText => "insertText:",

            // Deleting content
            Self::DeleteBackward => "deleteBackward:",
            Self::DeleteBackwardByDecomposingPreviousCharacter => {
                "deleteBackwardByDecomposingPreviousCharacter:"
            },
            Self::DeleteForward => "deleteForward:",
            Self::DeleteToBeginningOfLine => "deleteToBeginningOfLine:",
            Self::DeleteToBeginningOfParagraph => "deleteToBeginningOfParagraph:",
            Self::DeleteToEndOfLine => "deleteToEndOfLine:",
            Self::DeleteToEndOfParagraph => "deleteToEndOfParagraph:",
            Self::DeleteWordBackward => "deleteWordBackward:",
            Self::DeleteWordForward => "deleteWordForward:",
            Self::Yank => "yank:",

            // Moving the insertion pointer
            Self::MoveBackward => "moveBackward:",
            Self::MoveDown => "moveDown:",
            Self::MoveForward => "moveForward:",
            Self::MoveLeft => "moveLeft:",
            Self::MoveRight => "moveRight:",
            Self::MoveUp => "moveUp:",

            // Modifying the selection
            Self::MoveBackwardAndModifySelection => "moveBackwardAndModifySelection:",
            Self::MoveDownAndModifySelection => "moveDownAndModifySelection:",
            Self::MoveForwardAndModifySelection => "moveForwardAndModifySelection:",
            Self::MoveLeftAndModifySelection => "moveLeftAndModifySelection:",
            Self::MoveRightAndModifySelection => "moveRightAndModifySelection:",
            Self::MoveUpAndModifySelection => "moveUpAndModifySelection:",

            // Scrolling content
            Self::ScrollPageDown => "scrollPageDown:",
            Self::ScrollPageUp => "scrollPageUp:",
            Self::ScrollLineDown => "scrollLineDown:",
            Self::ScrollLineUp => "scrollLineUp:",
            Self::ScrollToBeginningOfDocument => "scrollToBeginningOfDocument:",
            Self::ScrollToEndOfDocument => "scrollToEndOfDocument:",
            Self::PageDown => "pageDown:",
            Self::PageUp => "pageUp:",
            Self::PageUpAndModifySelection => "pageUpAndModifySelection:",
            Self::PageDownAndModifySelection => "pageDownAndModifySelection:",
            Self::CenterSelectionInVisibleArea => "centerSelectionInVisibleArea:",

            // Transposing elements
            Self::Transpose => "transpose:",
            Self::TransposeWords => "transposeWords:",

            // Indenting content
            Self::Indent => "indent:",

            // Cancelling operations
            Self::CancelOperation => "cancelOperation:",

            // Supporting quickLook
            Self::QuickLookPreviewItems => "quickLookPreviewItems:",

            // Supporting writing direction
            Self::MakeBaseWritingDirectionLeftToRight => "makeBaseWritingDirectionLeftToRight:",
            Self::MakeBaseWritingDirectionNatural => "makeBaseWritingDirectionNatural:",
            Self::MakeBaseWritingDirectionRightToLeft => "makeBaseWritingDirectionRightToLeft:",
            Self::MakeTextWritingDirectionLeftToRight => "makeTextWritingDirectionLeftToRight:",
            Self::MakeTextWritingDirectionNatural => "makeTextWritingDirectionNatural:",
            Self::MakeTextWritingDirectionRightToLeft => "makeTextWritingDirectionRightToLeft:",

            // Changing captilization
            Self::CapitalizeWord => "capitalizeWord:",
            Self::ChangeCaseOfLetter => "changeCaseOfLetter:",
            Self::LowercaseWord => "lowercaseWord:",
            Self::UppercaseWord => "uppercaseWord:",

            // Moving the selection in documents
            Self::MoveToBeginningOfDocument => "moveToBeginningOfDocument:",
            Self::MoveToBeginningOfDocumentAndModifySelection => {
                "moveToBeginningOfDocumentAndModifySelection:"
            },
            Self::MoveToEndOfDocument => "moveToEndOfDocument:",
            Self::MoveToEndOfDocumentAndModifySelection => "moveToEndOfDocumentAndModifySelection:",

            // Moving the selection in paragraphs
            Self::MoveParagraphBackwardAndModifySelection => {
                "moveParagraphBackwardAndModifySelection:"
            },
            Self::MoveParagraphForwardAndModifySelection => {
                "moveParagraphForwardAndModifySelection:"
            },
            Self::MoveToBeginningOfParagraph => "moveToBeginningOfParagraph:",
            Self::MoveToBeginningOfParagraphAndModifySelection => {
                "moveToBeginningOfParagraphAndModifySelection:"
            },
            Self::MoveToEndOfParagraph => "moveToEndOfParagraph:",
            Self::MoveToEndOfParagraphAndModifySelection => {
                "moveToEndOfParagraphAndModifySelection:"
            },

            // Moving the selection in lines of text
            Self::MoveToBeginningOfLine => "moveToBeginningOfLine:",
            Self::MoveToBeginningOfLineAndModifySelection => {
                "moveToBeginningOfLineAndModifySelection:"
            },
            Self::MoveToEndOfLine => "moveToEndOfLine:",
            Self::MoveToEndOfLineAndModifySelection => "moveToEndOfLineAndModifySelection:",
            Self::MoveToLeftEndOfLine => "moveToLeftEndOfLine:",
            Self::MoveToLeftEndOfLineAndModifySelection => "moveToLeftEndOfLineAndModifySelection:",
            Self::MoveToRightEndOfLine => "moveToRightEndOfLine:",
            Self::MoveToRightEndOfLineAndModifySelection => {
                "moveToRightEndOfLineAndModifySelection:"
            },

            // Changing the selection
            Self::SelectAll => "selectAll:",
            Self::SelectLine => "selectLine:",
            Self::SelectParagraph => "selectParagraph:",
            Self::SelectWord => "selectWord:",

            // Supporting marked selections
            Self::SetMark => "setMark:",
            Self::SelectToMark => "selectToMark:",
            Self::DeleteToMark => "deleteToMark:",
            Self::SwapWithMark => "swapWithMark:",

            // Supporting autocomplete
            Self::Complete => "complete:",

            // Moving selection by word boundaries
            Self::MoveWordBackward => "moveWordBackward:",
            Self::MoveWordBackwardAndModifySelection => "moveWordBackwardAndModifySelection:",
            Self::MoveWordForward => "moveWordForward:",
            Self::MoveWordForwardAndModifySelection => "moveWordForwardAndModifySelection:",
            Self::MoveWordLeft => "moveWordLeft:",
            Self::MoveWordLeftAndModifySelection => "moveWordLeftAndModifySelection:",
            Self::MoveWordRight => "moveWordRight:",
            Self::MoveWordRightAndModifySelection => "moveWordRightAndModifySelection:",

            // Instance methods
            Self::ShowContextMenuForSelection => "showContextMenuForSelection:",

            // Other
            Self::Noop => "noop:",
            Self::Other(s) => s,
        }
    }
}

import { useAuth } from '@/hooks/useAuth';
import { useEmails } from '@/hooks/useEmails';
import { useUpdateEmail } from '@/hooks/useUpdateEmail';
import type { UserEmail } from '@/types/user';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';

import { Badge } from './Badge';
import { Button } from './Button';
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from './Select';
import { Skeleton } from './Skeleton';
import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
} from './ui/card';

export function UserEmails() {
    const { user } = useAuth();
    const { data: emails = [], isLoading } = useEmails();
    const { mutate: updateEmail, isPending, isSuccess } = useUpdateEmail();

    const [selectedEmail, setSelectedEmail] = useState('');
    const [originalEmail, setOriginalEmail] = useState('');
    const [hasChanges, setHasChanges] = useState(false);

    // Set selected email from user context on initial load
    useEffect(() => {
        if (user?.email && originalEmail === '') {
            setSelectedEmail(user.email);
            setOriginalEmail(user.email);
        } else if (emails.length > 0 && originalEmail === '' && !user?.email) {
            setSelectedEmail(emails[0].email);
            setOriginalEmail(emails[0].email);
        }
    }, [user?.email, emails, originalEmail]);

    // Track if email has changed from the original
    useEffect(() => {
        setHasChanges(selectedEmail !== originalEmail && selectedEmail !== '');
    }, [selectedEmail, originalEmail]);

    // Update original email when user email changes (after successful save)
    useEffect(() => {
        if (isSuccess && selectedEmail) {
            setOriginalEmail(selectedEmail);
            toast.success(`Email updated to ${selectedEmail}`);
        }
    }, [isSuccess, selectedEmail]);

    const handleSave = () => {
        if (hasChanges) {
            updateEmail({ email: selectedEmail });
        }
    };

    return (
        <Card className="mb-6">
            <CardHeader>
                <CardTitle>Email address</CardTitle>
                <CardDescription>
                    Email address used for account-related notifications
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="space-y-4">
                        <Skeleton className="h-10 w-full rounded-md" />
                    </div>
                ) : (
                    <div className="space-y-4">
                        <Select
                            value={selectedEmail}
                            onValueChange={(value) => setSelectedEmail(value)}
                        >
                            <SelectTrigger id="email-select">
                                <SelectValue placeholder="Select an email" />
                            </SelectTrigger>
                            <SelectContent>
                                {emails.map((email: UserEmail) => (
                                    <SelectItem
                                        key={email.email}
                                        value={email.email}
                                    >
                                        <div className="flex items-center gap-2">
                                            {email.email}
                                            {email.verified && (
                                                <Badge variant="success">
                                                    verified
                                                </Badge>
                                            )}
                                            {email.primary && (
                                                <Badge variant="default">
                                                    primary
                                                </Badge>
                                            )}
                                        </div>
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>
                )}
            </CardContent>
            <CardFooter>
                <Button
                    onClick={handleSave}
                    disabled={!hasChanges || isPending || isLoading}
                    variant="primary"
                    isLoading={isPending}
                >
                    Save Changes
                </Button>
            </CardFooter>
        </Card>
    );
}
